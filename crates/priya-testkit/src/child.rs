//! [`LoopChild`]: a test re-runs its own binary as a child process that does
//! something in a loop until killed, to check what survives a SIGKILL.
//!
//! The child is an `#[ignore]`d test in the same file. It reports progress
//! with [`ack`] (one line on stdout per finished unit of work) and stops by
//! itself once its stdin closes ([`until_stdin_closes`]), so it never
//! outlives the test. Dropping a `LoopChild` kills it.
//!
//! ```ignore
//! let mut child = LoopChild::spawn("write_loop_child", &[("DIR", dir.path().as_os_str())], dir.path());
//! let acked = child.wait_for_acks(3);
//! let late = child.kill(); // any that arrived after the wait
//!
//! #[test]
//! #[ignore = "helper process"]
//! fn write_loop_child() {
//!     let Some(dir) = std::env::var_os("DIR") else { return };
//!     let stop = priya_testkit::until_stdin_closes();
//!     while !stop.load(Ordering::Relaxed) { /* work */ priya_testkit::ack(n); }
//! }
//! ```

use std::ffi::OsStr;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

const ACK: &str = "acked ";

/// The child's side: reports one finished unit of work.
pub fn ack(what: impl Display) {
    let mut stdout = std::io::stdout();
    // A closed stdout means the parent is gone; the loop stops on its own.
    let _ = writeln!(stdout, "{ACK}{what}");
    let _ = stdout.flush();
}

/// The child's side: becomes true once the parent closes stdin or goes away.
pub fn until_stdin_closes() -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    std::thread::spawn({
        let stop = stop.clone();
        move || {
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            stop.store(true, Ordering::Relaxed);
        }
    });
    stop
}

/// A child process running one ignored test of this binary in a loop.
pub struct LoopChild {
    child: Child,
    _stdin: ChildStdin,
    acks: mpsc::Receiver<String>,
    reader: Option<JoinHandle<()>>,
    stderr: PathBuf,
}

impl LoopChild {
    /// Starts the ignored test `test` with `env`. Its stderr goes to a file
    /// in `scratch`, shown if it exits early.
    pub fn spawn(test: &str, env: &[(&str, &OsStr)], scratch: &Path) -> Self {
        let stderr = scratch.join(format!("{test}-{}.stderr", std::process::id()));
        let mut command = Command::new(std::env::current_exe().expect("the test binary's path"));
        command
            .args([
                test,
                "--exact",
                "--include-ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(&stderr).expect("a file for the child's stderr"));
        for (name, value) in env {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("the child starts");
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        let (sender, acks) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(what) = line.strip_prefix(ACK)
                    && sender.send(what.to_owned()).is_err()
                {
                    return;
                }
            }
        });
        Self {
            child,
            _stdin: stdin,
            acks,
            reader: Some(reader),
            stderr,
        }
    }

    /// Fails the test, with the child's stderr, if the child has exited.
    pub fn assert_running(&mut self) {
        if let Some(status) = self.child.try_wait().expect("the child's status") {
            let mut stderr = String::new();
            File::open(&self.stderr)
                .and_then(|mut f| f.read_to_string(&mut stderr))
                .expect("the child's stderr");
            panic!("the child exited early ({status}):\n{stderr}");
        }
    }

    /// Waits for `count` more acknowledgements and returns them.
    pub fn wait_for_acks(&mut self, count: usize) -> Vec<String> {
        let mut acked = Vec::with_capacity(count);
        while acked.len() < count {
            match self.acks.recv_timeout(Duration::from_secs(60)) {
                Ok(what) => acked.push(what),
                Err(_) => {
                    self.assert_running();
                    panic!("the child acknowledged nothing for 60 s");
                }
            }
        }
        acked
    }

    /// SIGKILLs the child (no chance to clean up) and returns the
    /// acknowledgements that arrived since the last wait.
    pub fn kill(mut self) -> Vec<String> {
        self.assert_running();
        self.child.kill().expect("the child is killed");
        self.child.wait().expect("the killed child is reaped");
        if let Some(reader) = self.reader.take() {
            reader.join().expect("the stdout reader doesn't panic");
        }
        self.acks.try_iter().collect()
    }
}

impl Drop for LoopChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
