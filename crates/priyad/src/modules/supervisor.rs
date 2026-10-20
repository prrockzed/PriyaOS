//! One task per enabled module: start it, look after it, restart it with
//! a growing pause when it fails, and give up when it keeps failing. Every
//! step is recorded: `module.started`, `module.stopped`, `module.crashed`.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use priya_api::events::{
    Actor, EventPayload, ModuleCrashed, ModuleStarted, ModuleStopped, NewEvent,
};
use priya_api::module::ModuleManifest;
use priya_api::peer::Peer;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use super::Context;
use super::discovery::Discovered;
use super::launch::{Exit, launch};
use super::policy::{Decision, Failures};
use super::session::{Outcome, Session, StartError, kill, stopped as stop_asked};

/// Where a module is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Starting,
    Running,
    /// Failed; waiting to start again.
    Restarting,
    Stopping,
    Stopped,
    /// Failed too often, or can't work: the host gave up on it.
    Failed,
}

/// What can be seen of a module from outside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub state: State,
    pub pid: Option<u32>,
    /// How many times it has been restarted.
    pub restarts: u32,
    pub last_error: Option<String>,
}

/// The handle on a module's task.
pub struct Supervisor {
    status: watch::Receiver<Status>,
    /// `Some(reason)` asks it to stop.
    stop: watch::Sender<Option<String>>,
    /// The running module's line, for calls.
    peer: Arc<Mutex<Option<Peer>>>,
    task: JoinHandle<()>,
}

impl Supervisor {
    /// Starts looking after `module`.
    pub fn start(context: Arc<Context>, module: Discovered) -> Self {
        let (status_tx, status) = watch::channel(Status {
            state: State::Starting,
            pid: None,
            restarts: 0,
            last_error: None,
        });
        let (stop, stop_rx) = watch::channel(None);
        let peer = Arc::new(Mutex::new(None));
        let task = tokio::spawn(supervise(
            context,
            module,
            Watch {
                status: status_tx,
                stop: stop_rx,
                peer: peer.clone(),
            },
        ));
        Self {
            status,
            stop,
            peer,
            task,
        }
    }

    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    /// The running module's line, if it is running.
    pub fn peer(&self) -> Option<Peer> {
        self.peer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Whether it has stopped for good (stopped or given up on).
    pub fn is_done(&self) -> bool {
        self.task.is_finished()
    }

    /// Asks it to stop, recording `reason`, and waits until it has.
    pub async fn stop(self, reason: &str) {
        self.stop.send_replace(Some(reason.to_owned()));
        if let Err(e) = self.task.await
            && e.is_panic()
        {
            warn!("a module's supervisor panicked");
        }
    }
}

/// The supervisor task's side of the handle.
struct Watch {
    status: watch::Sender<Status>,
    stop: watch::Receiver<Option<String>>,
    peer: Arc<Mutex<Option<Peer>>>,
}

impl Watch {
    fn set(&self, change: impl FnOnce(&mut Status)) {
        self.status.send_modify(change);
    }

    fn set_peer(&self, peer: Option<Peer>) {
        *self.peer.lock().unwrap_or_else(PoisonError::into_inner) = peer;
    }

    fn stop_reason(&self) -> Option<String> {
        self.stop.borrow().clone()
    }
}

async fn supervise(context: Arc<Context>, module: Discovered, mut watch: Watch) {
    let manifest = Arc::new(module.manifest.clone());
    let name = manifest.name().to_owned();
    let mut failures = Failures::new(context.policy);
    let mut restarts = 0;
    loop {
        if let Some(reason) = watch.stop_reason() {
            return stopped(&context, &watch, &name, &reason).await;
        }
        watch.set(|s| {
            s.state = State::Starting;
            s.pid = None;
        });
        let started = Instant::now();
        let failure = match run_once(&context, &module, &manifest, &mut watch).await {
            Run::Stopped(reason) => return stopped(&context, &watch, &name, &reason).await,
            Run::Refused(error) => {
                give_up(
                    &context,
                    &watch,
                    &manifest,
                    restarts,
                    Exit::default(),
                    None,
                    Some(error),
                )
                .await;
                return;
            }
            Run::Failed(failure) => failure,
        };
        let (exit, tail, error) = failure;
        let Decision::RestartAfter(pause) = failures.record(started, Instant::now()) else {
            give_up(&context, &watch, &manifest, restarts, exit, tail, error).await;
            return;
        };
        record_crash(&context, &name, restarts, &exit, tail, error.clone(), false).await;
        restarts += 1;
        watch.set(|s| {
            s.state = State::Restarting;
            s.pid = None;
            s.restarts = restarts;
            s.last_error = Some(describe(&exit, error.as_deref()));
        });
        info!(module = %name, ?pause, "restarting a module that failed");
        tokio::select! {
            () = tokio::time::sleep(pause) => {}
            () = stop_asked(&mut watch.stop) => {}
        }
    }
}

/// How one run went.
enum Run {
    Stopped(String),
    /// It isn't what its manifest says, or can't run at all.
    Refused(String),
    /// It failed: how it ended, its stderr, and what went wrong.
    Failed((Exit, Option<String>, Option<String>)),
}

async fn run_once(
    context: &Arc<Context>,
    module: &Discovered,
    manifest: &Arc<ModuleManifest>,
    watch: &mut Watch,
) -> Run {
    let name = manifest.name();
    let launched = match launch(module, &context.builtins, context.spawn) {
        Ok(launched) => launched,
        // A program that can't start won't start next time either.
        Err(reason) => return Run::Refused(reason),
    };
    let mut session = match Session::start(
        manifest.clone(),
        launched,
        context.services.clone(),
        &context.timings,
    )
    .await
    {
        Ok(session) => session,
        Err((error, mut running)) => {
            let exit = kill(&mut running, &context.timings).await;
            let tail = running.stderr_tail().await;
            return match error {
                StartError::Refused(reason) => Run::Refused(reason),
                StartError::Failed(reason) => Run::Failed((exit, tail, Some(reason))),
            };
        }
    };
    let pid = session.pid();
    record(
        context,
        name,
        &ModuleStarted {
            module: name.to_owned(),
            version: manifest.module.version.clone(),
            pid,
        },
    )
    .await;
    info!(module = %name, ?pid, "module started");
    watch.set_peer(Some(session.peer().clone()));
    watch.set(|s| {
        s.state = State::Running;
        s.pid = pid;
    });
    let outcome = session.watch(&mut watch.stop, &context.timings).await;
    watch.set_peer(None);
    match outcome {
        Outcome::Stop => {
            watch.set(|s| s.state = State::Stopping);
            let _ = session.stop(&context.timings).await;
            Run::Stopped(watch.stop_reason().unwrap_or_default())
        }
        Outcome::Failed(error) => {
            let (exit, tail) = session.end(&context.timings).await;
            Run::Failed((exit, tail, error))
        }
    }
}

async fn stopped(context: &Context, watch: &Watch, name: &str, reason: &str) {
    record(
        context,
        name,
        &ModuleStopped {
            module: name.to_owned(),
            reason: reason.to_owned(),
        },
    )
    .await;
    info!(module = %name, %reason, "module stopped");
    watch.set(|s| {
        s.state = State::Stopped;
        s.pid = None;
    });
}

async fn give_up(
    context: &Context,
    watch: &Watch,
    manifest: &ModuleManifest,
    restarts: u32,
    exit: Exit,
    tail: Option<String>,
    error: Option<String>,
) {
    let name = manifest.name();
    let described = describe(&exit, error.as_deref());
    warn!(module = %name, "gave up on a module: {described}");
    record_crash(context, name, restarts, &exit, tail, error, true).await;
    watch.set(|s| {
        s.state = State::Failed;
        s.pid = None;
        s.last_error = Some(described);
    });
}

async fn record_crash(
    context: &Context,
    name: &str,
    restarts: u32,
    exit: &Exit,
    stderr_tail: Option<String>,
    error: Option<String>,
    gave_up: bool,
) {
    let crashed = ModuleCrashed {
        module: name.to_owned(),
        exit_code: exit.code,
        signal: exit.signal,
        restarts,
        error: error.or_else(|| exit.error.clone()),
        stderr_tail,
        gave_up,
    };
    record(context, name, &crashed).await;
}

/// A failure in one sentence.
fn describe(exit: &Exit, error: Option<&str>) -> String {
    if let Some(error) = error.or(exit.error.as_deref()) {
        return error.to_owned();
    }
    match (exit.code, exit.signal) {
        (_, Some(signal)) => format!("it was killed by signal {signal}"),
        (Some(code), None) => format!("it exited with code {code}"),
        (None, None) => "it stopped unexpectedly".into(),
    }
}

async fn record<P: EventPayload>(context: &Context, name: &str, payload: &P) {
    let event = match NewEvent::new(format!("module:{name}"), Actor::System, payload) {
        Ok(event) => event,
        Err(e) => {
            warn!(error = %e, "couldn't build a module event");
            return;
        }
    };
    if let Err(e) = context.services.record(event).await {
        warn!(module = %name, error = %e, "couldn't record a module event");
    }
}
