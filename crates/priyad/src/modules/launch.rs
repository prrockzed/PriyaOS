//! Starting a module: as its own process speaking on stdio, or as a builtin
//! on in-process channels. Either way the host gets the same two channels
//! of messages and a [`Running`] handle, so everything above this file is
//! shared by both kinds (02-core §5.3).

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};

use priya_api::framing::lsp::{read_messages, write_messages};
use priya_api::module::ModuleKind;
use priya_api::peer::{Ended, Inbound};
use priya_module_sdk::BuiltinMain;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
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
const STDERR_DRAIN: std::time::Duration = std::time::Duration::from_millis(500);

/// How a process module is started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spawn {
    /// A plain child of priyad.
    Direct,
    /// In a transient systemd scope of its own, `priya-module-<name>`, so
    /// its resources are accounted (and later limited) separately.
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

/// A started module, with its two channels of messages.
pub struct Launched {
    pub inbound: mpsc::Receiver<Inbound>,
    pub outbound: mpsc::Sender<Vec<u8>>,
    pub running: Running,
}

/// A module that is running (or has just ended): how to stop it and how it
/// ended.
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
    Process(u32),
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

    /// Asks it to stop: SIGTERM for a process. A builtin is stopped at once.
    pub fn terminate(&self) {
        self.signal(rustix::process::Signal::TERM);
    }

    /// Stops it now: SIGKILL for a process.
    pub fn kill(&self) {
        self.signal(rustix::process::Signal::KILL);
    }

    fn signal(&self, signal: rustix::process::Signal) {
        match &self.stopper {
            Stopper::Process(pid) => {
                let pid = i32::try_from(*pid)
                    .ok()
                    .and_then(rustix::process::Pid::from_raw);
                if let Some(pid) = pid {
                    // It may have exited already; nothing to do then.
                    let _ = rustix::process::kill_process(pid, signal);
                }
            }
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

/// Starts `module`. A builtin must be in `builtins`.
pub fn launch(module: &Discovered, builtins: &Builtins, spawn: Spawn) -> Result<Launched, String> {
    match module.manifest.module.kind {
        ModuleKind::Builtin => builtin(module.manifest.name(), builtins),
        ModuleKind::Process => process(module, spawn),
    }
}

fn builtin(name: &str, builtins: &Builtins) -> Result<Launched, String> {
    let entry = builtins
        .get(name)
        .ok_or_else(|| format!("this priyad has no builtin module named `{name}`"))?;
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
        let ended = match task.await {
            Ok(Ended::Closed) => Exit::default(),
            Ok(Ended::Broken(reason)) => Exit {
                error: Some(reason),
                ..Exit::default()
            },
            Err(e) if e.is_panic() => Exit {
                error: Some("it panicked".into()),
                ..Exit::default()
            },
            Err(_) => Exit {
                error: Some("it was stopped".into()),
                ..Exit::default()
            },
        };
        let _ = exited.send(ended);
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

fn process(module: &Discovered, spawn: Spawn) -> Result<Launched, String> {
    let name = module.manifest.name().to_owned();
    let exec = module
        .exec
        .as_ref()
        .ok_or_else(|| "it has no program to run".to_owned())?;
    let mut command = match spawn {
        Spawn::Direct => tokio::process::Command::new(exec),
        Spawn::SystemdScope => {
            let mut command = tokio::process::Command::new("systemd-run");
            command
                .args(["--user", "--scope", "--collect", "--quiet"])
                .arg(format!("--unit=priya-module-{name}"))
                .arg("--")
                .arg(exec);
            command
        }
    };
    let mut child = command
        .current_dir(&module.dir)
        .env("PRIYA_MODULE_NAME", &name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("{} couldn't be started: {e}", exec.display()))?;
    let pid = child.id().ok_or_else(|| "it exited at once".to_owned())?;
    let (stdin, stdout, stderr) =
        match (child.stdin.take(), child.stdout.take(), child.stderr.take()) {
            (Some(stdin), Some(stdout), Some(stderr)) => (stdin, stdout, stderr),
            _ => return Err("its stdio couldn't be connected".into()),
        };
    let (to_host, inbound) = mpsc::channel(QUEUE);
    let (outbound, to_module) = mpsc::channel(QUEUE);
    tokio::spawn(read_messages(stdout, to_host));
    tokio::spawn(write_messages(stdin, to_module));
    let tail = Arc::new(Mutex::new(VecDeque::new()));
    let stderr = tokio::spawn(log_stderr(name, stderr, tail.clone()));
    let (exited, exit) = oneshot::channel();
    tokio::spawn(async move {
        let ended = match child.wait().await {
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
        let _ = exited.send(ended);
    });
    Ok(Launched {
        inbound,
        outbound,
        running: Running {
            pid: Some(pid),
            stopper: Stopper::Process(pid),
            exited: exit,
            exit: None,
            tail,
            stderr: Some(stderr),
        },
    })
}

/// Logs each stderr line under the module's name and keeps the last few.
/// Any bytes are fine; a very long line is taken in pieces.
async fn log_stderr(
    name: String,
    stderr: tokio::process::ChildStderr,
    tail: Arc<Mutex<VecDeque<String>>>,
) {
    let mut reader = BufReader::new(stderr);
    let mut buffer = Vec::new();
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
        info!(target: "module", module = %name, "{line}");
        let mut tail = tail.lock().unwrap_or_else(PoisonError::into_inner);
        if tail.len() == TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line.chars().take(TAIL_LINE_CHARS).collect());
    }
}
