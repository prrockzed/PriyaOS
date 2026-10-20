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
use std::os::unix::net::UnixStream;
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
}

impl TestDaemon {
    /// Starts `priyad` in a new temporary home and waits until it answers.
    pub fn start(priyad: impl Into<PathBuf>) -> io::Result<Self> {
        let home = TempPriyaHome::new()?;
        let mut daemon = Self {
            home,
            priyad: priyad.into(),
            child: None,
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
            // Never report to a systemd the tests happen to run under.
            .env_remove("NOTIFY_SOCKET")
            .env("NO_COLOR", "1");
        command
    }

    /// Sends SIGTERM, as systemd does, and waits for priyad to exit.
    pub fn stop(&mut self) -> io::Result<ExitStatus> {
        let Some(mut child) = self.child.take() else {
            return Err(io::Error::other("priyad isn't running"));
        };
        let pid = rustix::process::Pid::from_child(&child);
        rustix::process::kill_process(pid, rustix::process::Signal::TERM)?;
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

    /// Starts priyad (again) in the same home, and waits until it answers.
    pub fn restart(&mut self) -> io::Result<()> {
        if self.child.is_some() {
            self.stop()?;
        }
        let log = File::options()
            .create(true)
            .append(true)
            .open(self.log_file())?;
        let mut child = self
            .command(&self.priyad)
            .arg("--foreground")
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?;
        let deadline = Instant::now() + PATIENCE;
        while UnixStream::connect(self.socket()).is_err() {
            let exited = child.try_wait()?;
            if exited.is_some() || Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::other(format!(
                    "priyad didn't start ({}); its log:\n{}",
                    exited.map_or("no answer in time".to_owned(), |s| s.to_string()),
                    self.log()
                )));
            }
            std::thread::sleep(POLL);
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
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
