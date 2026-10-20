//! Starting a module: as its own process speaking on stdio, or as a builtin
//! on in-process channels. Either way the host gets the same two channels
//! of messages and a [`Running`] handle, so everything above this file is
//! shared by both kinds (02-core §5.3).

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use priya_api::framing::lsp::{read_messages, write_messages};
use priya_api::module::ModuleKind;
use priya_api::peer::{Ended, Inbound};
use priya_module_sdk::BuiltinMain;
use rustix::process::{Pid, Signal, kill_process_group};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::{mpsc, oneshot};
use tokio::task::AbortHandle;
use tracing::info;

use super::discovery::Discovered;

/// The builtin modules compiled into priyad, by name.
pub type Builtins = BTreeMap<&'static str, BuiltinMain>;

/// Messages waiting in each direction.
const QUEUE: usize = 64;
/// How much of a module's stderr is kept for `module.crashed`.
const TAIL_LINES: usize = 20;
const TAIL_LINE_CHARS: usize = 300;
/// How long to wait for the last stderr lines of a module that ended.
const STDERR_DRAIN: Duration = Duration::from_millis(500);
/// The most stderr lines logged per second; the rest are counted.
const LOG_LINES_PER_SECOND: u32 = 50;

/// How a process module is started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spawn {
    /// A plain child of priyad.
    Direct,
    /// In a transient systemd scope of its own, so its resources are
    /// accounted (and later limited) separately.
    SystemdScope,
}

impl Spawn {
    /// A systemd scope when this user has a systemd user manager to ask
    /// (its private socket is in `$XDG_RUNTIME_DIR`), else a plain child.
    pub fn detect() -> Self {
        let manager = std::env::var_os("XDG_RUNTIME_DIR")
            .map(|dir| Path::new(&dir).join("systemd/private"))
            .is_some_and(|socket| socket.exists());
        if cfg!(target_os = "linux") && manager {
            Self::SystemdScope
        } else {
            Self::Direct
        }
    }
}

/// How a module ended.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exit {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    /// What went wrong, for a builtin that broke or panicked.
    pub error: Option<String>,
}

/// Why a module couldn't be started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// It never will be: no such builtin, a program that is missing or
    /// can't be run.
    Impossible(String),
    /// It might next time (too many open files, out of memory…).
    Failed(String),
}

/// A started module, with its two channels of messages.
pub struct Launched {
    pub inbound: mpsc::Receiver<Inbound>,
    pub outbound: mpsc::Sender<Vec<u8>>,
    pub running: Running,
}

/// A module that is running (or has just ended): how to stop it and how it
/// ended. Dropping it stops a module that is still running.
pub struct Running {
    pid: Option<u32>,
    stopper: Stopper,
    exited: oneshot::Receiver<Exit>,
    exit: Option<Exit>,
    tail: Arc<Mutex<VecDeque<String>>>,
    /// The task reading stderr, which ends at end of file.
    stderr: Option<tokio::task::JoinHandle<()>>,
}

enum Stopper {
    /// Signals for the task that owns the child: it knows whether the
    /// child has been reaped, so a signal never reaches a reused pid.
    Process(mpsc::UnboundedSender<Signal>),
    Task(AbortHandle),
}

impl Running {
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Waits until it has ended and says how. Safe to call again, and to
    /// cancel.
    pub async fn exited(&mut self) -> Exit {
        if self.exit.is_none() {
            let exit = (&mut self.exited).await.unwrap_or_else(|_| Exit {
                error: Some("priyad lost track of it".into()),
                ..Exit::default()
            });
            self.exit = Some(exit);
        }
        self.exit.clone().unwrap_or_default()
    }

    /// Asks it to stop: SIGTERM to a process and its children. A builtin is
    /// stopped at once.
    pub fn terminate(&self) {
        self.signal(Signal::TERM);
    }

    /// Stops it now: SIGKILL to a process and its children.
    pub fn kill(&self) {
        self.signal(Signal::KILL);
    }

    fn signal(&self, signal: Signal) {
        match &self.stopper {
            // Gone already when the waiter has finished: nothing to do.
            Stopper::Process(signals) => drop(signals.send(signal)),
            Stopper::Task(task) => task.abort(),
        }
    }

    /// The last lines it wrote to stderr, if any. After it has exited,
    /// waits briefly for the last of them to be read.
    pub async fn stderr_tail(&mut self) -> Option<String> {
        if let Some(reader) = self.stderr.take() {
            let _ = tokio::time::timeout(STDERR_DRAIN, reader).await;
        }
        let tail = self.tail.lock().unwrap_or_else(PoisonError::into_inner);
        (!tail.is_empty()).then(|| tail.iter().cloned().collect::<Vec<_>>().join("\n"))
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if self.exit.is_none() {
            self.kill();
        }
    }
}

/// Starts `module`. A builtin must be in `builtins`.
pub fn launch(
    module: &Discovered,
    builtins: &Builtins,
    spawn: Spawn,
) -> Result<Launched, LaunchError> {
    match module.manifest.module.kind {
        ModuleKind::Builtin => builtin(module.manifest.name(), builtins),
        ModuleKind::Process => process(module, spawn),
    }
}

fn builtin(name: &str, builtins: &Builtins) -> Result<Launched, LaunchError> {
    let entry = builtins.get(name).ok_or_else(|| {
        LaunchError::Impossible(format!("this priyad has no builtin module named `{name}`"))
    })?;
    let (to_module, module_in) = mpsc::channel(QUEUE);
    let (module_out, from_module) = mpsc::channel(QUEUE);
    let (to_host, inbound) = mpsc::channel(QUEUE);
    let (outbound, from_host) = mpsc::channel(QUEUE);
    forward(from_module, to_host);
    forward(from_host, to_module);
    let task = tokio::spawn(entry(module_in, module_out));
    let stopper = Stopper::Task(task.abort_handle());
    let (exited, exit) = oneshot::channel();
    tokio::spawn(async move {
        let _ = exited.send(builtin_exit(task.await));
    });
    Ok(Launched {
        inbound,
        outbound,
        running: Running {
            pid: None,
            stopper,
            exited: exit,
            exit: None,
            tail: Arc::default(),
            stderr: None,
        },
    })
}

/// How a builtin's task ended, as an exit.
fn builtin_exit(ended: Result<Ended, tokio::task::JoinError>) -> Exit {
    let error = match ended {
        Ok(Ended::Closed) => None,
        Ok(Ended::Broken(reason)) => Some(reason),
        Err(e) if e.is_panic() => {
            let panic = e.into_panic();
            let message = panic
                .downcast_ref::<&str>()
                .map(ToString::to_string)
                .or_else(|| panic.downcast_ref::<String>().cloned());
            Some(match message {
                Some(message) => format!("it panicked: {message}"),
                None => "it panicked".into(),
            })
        }
        Err(_) => Some("it was stopped".into()),
    };
    Exit {
        error,
        ..Exit::default()
    }
}

/// Carries a builtin's messages across, as stdio would.
fn forward(mut from: mpsc::Receiver<Vec<u8>>, to: mpsc::Sender<Inbound>) {
    tokio::spawn(async move {
        while let Some(body) = from.recv().await {
            if to.send(Ok(body)).await.is_err() {
                break;
            }
        }
    });
}

/// Scopes started by this priyad, so each gets a name of its own.
static SCOPES: AtomicU64 = AtomicU64::new(0);

/// The command that starts a process module: the program itself, or
/// `systemd-run` around it. Returns the command and the program run.
fn command(module: &Discovered, exec: &Path, spawn: Spawn) -> (tokio::process::Command, PathBuf) {
    let name = module.manifest.name();
    let (mut command, program) = match spawn {
        Spawn::Direct => (tokio::process::Command::new(exec), exec.to_path_buf()),
        Spawn::SystemdScope => {
            let mut command = tokio::process::Command::new("systemd-run");
            // Unique per start: another priyad (a test's) may run a module
            // of the same name, and an old scope may still be going away.
            let n = SCOPES.fetch_add(1, Ordering::Relaxed);
            command
                .args(["--user", "--scope", "--collect", "--quiet"])
                .arg(format!(
                    "--unit=priya-module-{name}-{}-{n}",
                    std::process::id()
                ))
                .arg("--")
                .arg(exec);
            (command, PathBuf::from("systemd-run"))
        }
    };
    command
        .current_dir(&module.dir)
        .env("PRIYA_MODULE_NAME", name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // A group of its own, so stopping it stops what it started.
        .process_group(0)
        .kill_on_drop(true);
    (command, program)
}

fn process(module: &Discovered, spawn: Spawn) -> Result<Launched, LaunchError> {
    let name = module.manifest.name().to_owned();
    let exec = module
        .exec
        .as_ref()
        .ok_or_else(|| LaunchError::Impossible("it has no program to run".to_owned()))?;
    let (mut command, program) = command(module, exec, spawn);
    let mut child = command.spawn().map_err(|e| spawn_error(&program, &e))?;
    let pid = child
        .id()
        .ok_or_else(|| LaunchError::Failed("it exited at once".to_owned()))?;
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err(LaunchError::Failed(
            "its stdio couldn't be connected".into(),
        ));
    };
    let (to_host, inbound) = mpsc::channel(QUEUE);
    let (outbound, to_module) = mpsc::channel(QUEUE);
    tokio::spawn(read_messages(stdout, to_host));
    tokio::spawn(write_messages(stdin, to_module));
    let tail = Arc::new(Mutex::new(VecDeque::new()));
    let stderr = tokio::spawn(log_stderr(name, stderr, tail.clone()));
    let (signals, signals_in) = mpsc::unbounded_channel();
    let (exited, exit) = oneshot::channel();
    tokio::spawn(wait(child, pid, signals_in, exited));
    Ok(Launched {
        inbound,
        outbound,
        running: Running {
            pid: Some(pid),
            stopper: Stopper::Process(signals),
            exited: exit,
            exit: None,
            tail,
            stderr: Some(stderr),
        },
    })
}

/// Which spawn failures are worth retrying.
fn spawn_error(program: &Path, error: &io::Error) -> LaunchError {
    let message = format!("{} couldn't be started: {error}", program.display());
    // ENOEXEC: not a program this system can run (a bad #! line, say).
    let never = matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    ) || error.raw_os_error() == Some(8);
    if never {
        LaunchError::Impossible(message)
    } else {
        LaunchError::Failed(message)
    }
}

/// Owns the child: reaps it, and signals it (and its process group) only
/// while it hasn't been reaped, so its pid can't belong to anyone else. A
/// handle that goes away kills it.
async fn wait(
    mut child: Child,
    pid: u32,
    mut signals: mpsc::UnboundedReceiver<Signal>,
    exited: oneshot::Sender<Exit>,
) {
    let group = i32::try_from(pid).ok().and_then(Pid::from_raw);
    let mut listening = true;
    let status = loop {
        tokio::select! {
            status = child.wait() => break status,
            signal = signals.recv(), if listening => {
                let signal = signal.unwrap_or_else(|| {
                    listening = false;
                    Signal::KILL
                });
                if let Some(group) = group {
                    let _ = kill_process_group(group, signal);
                }
            }
        }
    };
    let exit = match status {
        Ok(status) => Exit {
            code: status.code(),
            signal: std::os::unix::process::ExitStatusExt::signal(&status),
            error: None,
        },
        Err(e) => Exit {
            error: Some(format!("priyad couldn't wait for it: {e}")),
            ..Exit::default()
        },
    };
    let _ = exited.send(exit);
}

/// Logs each stderr line under the module's name and keeps the last few.
/// Any bytes are fine; a very long line is taken in pieces; a module that
/// floods stderr has the excess counted, not logged.
async fn log_stderr(
    name: String,
    stderr: tokio::process::ChildStderr,
    tail: Arc<Mutex<VecDeque<String>>>,
) {
    let mut reader = BufReader::new(stderr);
    let mut buffer = Vec::new();
    let mut second = Instant::now();
    let (mut logged, mut dropped) = (0_u32, 0_u64);
    loop {
        buffer.clear();
        match (&mut reader)
            .take(8192)
            .read_until(b'\n', &mut buffer)
            .await
        {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&buffer);
        let line = line.trim_end_matches(['\n', '\r']);
        if second.elapsed() >= Duration::from_secs(1) {
            if dropped > 0 {
                info!(target: "module", module = %name, "({dropped} more lines not logged)");
            }
            (second, logged, dropped) = (Instant::now(), 0, 0);
        }
        if logged < LOG_LINES_PER_SECOND {
            info!(target: "module", module = %name, "{line}");
            logged += 1;
        } else {
            dropped += 1;
        }
        let mut tail = tail.lock().unwrap_or_else(PoisonError::into_inner);
        if tail.len() == TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line.chars().take(TAIL_LINE_CHARS).collect());
    }
}

#[cfg(test)]
mod tests {
    use priya_api::module::ModuleManifest;

    use super::*;

    fn module(dir: &Path) -> Discovered {
        Discovered {
            manifest: ModuleManifest::parse(
                "[module]\nname = \"echo\"\nversion = \"0.1.0\"\napi = \"v1\"\nkind = \"process\"\nexec = \"run\"\n",
            )
            .unwrap(),
            dir: dir.to_path_buf(),
            exec: Some(dir.join("run")),
            settings_schema: None,
        }
    }

    // covers: CORE-02
    // suite: unit
    #[test]
    fn a_systemd_scope_wraps_the_program_with_a_name_of_its_own() {
        let dir = Path::new("/opt/echo");
        let exec = dir.join("run");
        let (direct, program) = command(&module(dir), &exec, Spawn::Direct);
        assert_eq!(program, exec);
        assert_eq!(direct.as_std().get_program(), exec.as_os_str());
        assert_eq!(direct.as_std().get_current_dir(), Some(dir));
        let envs: Vec<_> = direct.as_std().get_envs().collect();
        assert!(
            envs.iter()
                .any(|(k, v)| *k == "PRIYA_MODULE_NAME" && *v == Some("echo".as_ref()))
        );

        let (scoped, program) = command(&module(dir), &exec, Spawn::SystemdScope);
        assert_eq!(program, Path::new("systemd-run"));
        let args: Vec<String> = scoped
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[..4], ["--user", "--scope", "--collect", "--quiet"]);
        assert!(args[4].starts_with(&format!("--unit=priya-module-echo-{}-", std::process::id())));
        assert_eq!(args[5..], ["--", "/opt/echo/run"]);
        let (again, _) = command(&module(dir), &exec, Spawn::SystemdScope);
        let again: Vec<String> = again
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_ne!(args[4], again[4], "each start has its own scope");
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn builtin_ends_and_spawn_failures_read_plainly() {
        assert_eq!(builtin_exit(Ok(Ended::Closed)), Exit::default());
        assert_eq!(
            builtin_exit(Ok(Ended::Broken("garbage".into())))
                .error
                .as_deref(),
            Some("garbage")
        );
        let panicked = tokio::spawn(async { panic!("out of cheese") }).await;
        assert_eq!(
            builtin_exit(panicked.map(|()| Ended::Closed))
                .error
                .as_deref(),
            Some("it panicked: out of cheese")
        );
        let task = tokio::spawn(std::future::pending::<Ended>());
        task.abort();
        assert_eq!(
            builtin_exit(task.await).error.as_deref(),
            Some("it was stopped")
        );

        let program = Path::new("/opt/echo/run");
        let missing = spawn_error(program, &io::Error::from(io::ErrorKind::NotFound));
        assert!(
            matches!(missing, LaunchError::Impossible(m) if m.starts_with("/opt/echo/run couldn't be started"))
        );
        assert!(matches!(
            spawn_error(program, &io::Error::from_raw_os_error(8)),
            LaunchError::Impossible(_)
        ));
        let busy = spawn_error(program, &io::Error::from_raw_os_error(24));
        assert!(
            matches!(busy, LaunchError::Failed(_)),
            "too many open files may pass"
        );
    }
}
