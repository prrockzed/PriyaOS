//! [`TestDaemon`]: the real `priyad` binary, run in a [`TempPriyaHome`] so
//! end-to-end tests drive it exactly as a user would, without touching the
//! real home. Dropping it kills the daemon.
//!
//! ```ignore
//! let mut daemon = TestDaemon::start(workspace_binary("priyad"))?;
//! let output = daemon.command(workspace_binary("priya")).arg("status").output()?;
//! daemon.stop()?; // SIGTERM, as systemd sends
//! daemon.restart()?; // same home, same history
//! ```

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use priya_api::client::{Client, ClientError};

use crate::TempPriyaHome;

/// How long priyad may take to start answering, or to stop.
const PATIENCE: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(20);

/// A binary built from this workspace, found next to the running test
/// (`target/<profile>/<name>`). `cargo test --workspace` builds them all;
/// a test of one package may need `cargo build -p <package>` first.
pub fn workspace_binary(name: &str) -> PathBuf {
    let test = std::env::current_exe().expect("the test knows its own path");
    // target/<profile>/deps/<test> → target/<profile>/<name>
    let path = test
        .parent()
        .and_then(Path::parent)
        .expect("the test runs from target/<profile>/deps")
        .join(name);
    assert!(
        path.is_file(),
        "{} isn't built; run `cargo build --workspace` first",
        path.display()
    );
    path
}

/// The real priyad in a temporary home.
pub struct TestDaemon {
    home: TempPriyaHome,
    priyad: PathBuf,
    child: Option<Child>,
    /// Extra environment for priyad itself, such as module timings.
    env: Vec<(String, String)>,
}

impl TestDaemon {
    /// Starts `priyad` in a new temporary home and waits until it answers.
    pub fn start(priyad: impl Into<PathBuf>) -> io::Result<Self> {
        Self::start_with(priyad, &[])
    }

    /// Starts `priyad` with extra environment, such as
    /// `PRIYA_MODULE_TIMINGS`, kept across restarts.
    pub fn start_with(priyad: impl Into<PathBuf>, env: &[(&str, &str)]) -> io::Result<Self> {
        let home = TempPriyaHome::new()?;
        let mut daemon = Self {
            home,
            priyad: priyad.into(),
            child: None,
            env: env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        };
        for folder in [daemon.defaults_dir(), daemon.system_dir()] {
            fs::create_dir_all(folder)?;
        }
        daemon.restart()?;
        Ok(daemon)
    }

    pub fn home(&self) -> &TempPriyaHome {
        &self.home
    }

    pub fn socket(&self) -> PathBuf {
        self.home.paths().runtime.join("priyad.sock")
    }

    /// The user's settings file, which `priya settings set` writes.
    pub fn user_settings(&self) -> PathBuf {
        self.home.paths().config.join("settings.toml")
    }

    /// Stands in for `/usr/share/priya/defaults`.
    pub fn defaults_dir(&self) -> PathBuf {
        self.base().join("defaults")
    }

    /// Stands in for `/etc/priya`.
    pub fn system_dir(&self) -> PathBuf {
        self.base().join("system")
    }

    /// The user's modules folder (`$XDG_DATA_HOME/priya/modules`), where
    /// tests put modules.
    pub fn modules_dir(&self) -> PathBuf {
        self.home.paths().data.join("modules")
    }

    /// Stands in for `/usr/lib/priya/modules`.
    pub fn system_modules_dir(&self) -> PathBuf {
        self.base().join("system-modules")
    }

    /// A typed API client connected to this daemon.
    pub async fn client(&self) -> Result<Client, ClientError> {
        Client::connect(&self.socket()).await
    }

    /// The process id, while it runs.
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    /// Everything priyad has logged, across restarts.
    pub fn log(&self) -> String {
        fs::read_to_string(self.log_file()).unwrap_or_default()
    }

    /// A command for `program` (the CLI, or a second priyad) with this
    /// daemon's home, so it finds the same socket and folders.
    pub fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        self.home.apply(&mut command);
        command
            .env("PRIYA_DEFAULTS_DIR", self.defaults_dir())
            .env("PRIYA_SYSTEM_DIR", self.system_dir())
            .env("PRIYA_SYSTEM_MODULES_DIR", self.system_modules_dir())
            // Never report to, or log into, a systemd the tests happen to
            // run under.
            .env_remove("NOTIFY_SOCKET")
            .env_remove("JOURNAL_STREAM")
            .env("NO_COLOR", "1");
        command
    }

    /// Sends SIGTERM, as systemd does, and waits for priyad to exit.
    pub fn stop(&mut self) -> io::Result<ExitStatus> {
        let Some(mut child) = self.child.take() else {
            return Err(io::Error::other("priyad isn't running"));
        };
        let pid = rustix::process::Pid::from_child(&child);
        if let Err(e) = rustix::process::kill_process(pid, rustix::process::Signal::TERM) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e.into());
        }
        let deadline = Instant::now() + PATIENCE;
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::other(format!(
                    "priyad didn't stop within {PATIENCE:?}; its log:\n{}",
                    self.log()
                )));
            }
            std::thread::sleep(POLL);
        }
    }

    /// Starts priyad (again) in the same home, and waits until it is
    /// ready: it says `READY=1`, as it would to systemd, once its settings
    /// are loaded and watched and its socket is served.
    pub fn restart(&mut self) -> io::Result<()> {
        if self.child.is_some() {
            self.stop()?;
        }
        let log = File::options()
            .create(true)
            .append(true)
            .open(self.log_file())?;
        let notify_path = self.base().join("notify");
        let _ = fs::remove_file(&notify_path);
        let notify = UnixDatagram::bind(&notify_path)?;
        notify.set_read_timeout(Some(POLL))?;
        let mut child = self
            .command(&self.priyad)
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .arg("--foreground")
            .env("NOTIFY_SOCKET", &notify_path)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?;
        let deadline = Instant::now() + PATIENCE;
        let mut said = [0_u8; 64];
        while !notify
            .recv(&mut said)
            .is_ok_and(|n| said[..n].starts_with(b"READY=1"))
        {
            let exited = child.try_wait()?;
            if exited.is_some() || Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::other(format!(
                    "priyad didn't start ({}); its log:\n{}",
                    exited.map_or("not ready in time".to_owned(), |s| s.to_string()),
                    self.log()
                )));
            }
            // recv waited up to POLL already.
        }
        self.child = Some(child);
        Ok(())
    }

    /// The folder holding the PriyaOS folders, private to this daemon.
    fn base(&self) -> PathBuf {
        self.home
            .paths()
            .data
            .parent()
            .expect("the data folder has a parent")
            .to_path_buf()
    }

    fn log_file(&self) -> PathBuf {
        self.base().join("priyad.log")
    }
}

impl Drop for TestDaemon {
    /// Stops priyad as systemd would (SIGTERM), so it stops its modules
    /// and a coverage run gets its profile; SIGKILL if it takes too long.
    fn drop(&mut self) {
        if self.child.is_some() && self.stop().is_ok() {
            return;
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
