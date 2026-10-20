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
use super::discovery::{Discovered, discover};
use super::launch::{Exit, LaunchError, launch};
use super::policy::{Decision, Failures};
use super::session::{Outcome, Session, StartError, kill, stopped as stop_asked};

/// The stop reason when the host goes away without saying why.
const HOST_GONE: &str = "priyad is stopping";

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
    /// The manifest of the copy running (or last run).
    manifest: Arc<Mutex<Arc<ModuleManifest>>>,
    task: JoinHandle<()>,
}

impl Supervisor {
    /// Starts looking after `module`, once `after` (an earlier copy still
    /// stopping) has finished.
    pub fn start(
        context: Arc<Context>,
        module: Discovered,
        after: Option<watch::Receiver<bool>>,
    ) -> Self {
        let (status_tx, status) = watch::channel(Status {
            state: State::Starting,
            pid: None,
            restarts: 0,
            last_error: None,
        });
        let (stop, stop_rx) = watch::channel(None);
        let peer = Arc::new(Mutex::new(None));
        let manifest = Arc::new(Mutex::new(Arc::new(module.manifest.clone())));
        let watch = Watch {
            status: status_tx,
            stop: stop_rx,
            peer: peer.clone(),
            manifest: manifest.clone(),
        };
        let task = tokio::spawn(async move {
            if let Some(mut before) = after {
                let _ = before.wait_for(|done| *done).await;
            }
            supervise(context, module, watch).await;
        });
        Self {
            status,
            stop,
            peer,
            manifest,
            task,
        }
    }

    /// Its status. A task that ended without saying it stopped (it
    /// panicked) counts as failed.
    pub fn status(&self) -> Status {
        let mut status = self.status.borrow().clone();
        if self.task.is_finished() && !matches!(status.state, State::Stopped | State::Failed) {
            status.state = State::Failed;
            status.pid = None;
            status.last_error = Some("priyad stopped looking after it".into());
        }
        status
    }

    /// The running module's line, if it is running.
    pub fn peer(&self) -> Option<Peer> {
        self.peer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The manifest of the copy it runs.
    pub fn manifest(&self) -> Arc<ModuleManifest> {
        self.manifest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
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
    manifest: Arc<Mutex<Arc<ModuleManifest>>>,
}

impl Watch {
    fn set(&self, change: impl FnOnce(&mut Status)) {
        self.status.send_modify(change);
    }

    fn set_peer(&self, peer: Option<Peer>) {
        *self.peer.lock().unwrap_or_else(PoisonError::into_inner) = peer;
    }

    /// Why to stop, if asked; a host that went away is a stop too.
    fn stop_reason(&self) -> Option<String> {
        if let Some(reason) = self.stop.borrow().clone() {
            return Some(reason);
        }
        self.stop
            .has_changed()
            .is_err()
            .then(|| HOST_GONE.to_owned())
    }
}

async fn supervise(context: Arc<Context>, mut module: Discovered, mut watch: Watch) {
    let name = module.manifest.name().to_owned();
    let mut failures = Failures::new(context.policy);
    let mut restarts = 0;
    loop {
        if let Some(reason) = watch.stop_reason() {
            return stopped(&context, &watch, &name, &reason).await;
        }
        if restarts > 0 {
            // It may have been upgraded or removed meanwhile.
            match rediscover(&context, &name).await {
                Some(found) => module = found,
                None => {
                    let gone = Failure::because("its folder is gone".into());
                    return give_up(&context, &watch, &name, restarts, gone).await;
                }
            }
        }
        let manifest = Arc::new(module.manifest.clone());
        *watch
            .manifest
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = manifest.clone();
        watch.set(|s| {
            s.state = State::Starting;
            s.pid = None;
        });
        let started = Instant::now();
        let failure = match run_once(&context, &module, &manifest, &mut watch).await {
            Run::Stopped(reason) => return stopped(&context, &watch, &name, &reason).await,
            Run::Refused(failure) => {
                return give_up(&context, &watch, &name, restarts, failure).await;
            }
            Run::Failed(failure) => failure,
        };
        let Decision::RestartAfter(pause) = failures.record(started, Instant::now()) else {
            return give_up(&context, &watch, &name, restarts, failure).await;
        };
        let described = failure.describe();
        record_crash(&context, &name, restarts, failure, false).await;
        restarts += 1;
        watch.set(|s| {
            s.state = State::Restarting;
            s.pid = None;
            s.restarts = restarts;
            s.last_error = Some(described);
        });
        info!(module = %name, ?pause, "restarting a module that failed");
        tokio::select! {
            () = tokio::time::sleep(pause) => {}
            () = stop_asked(&mut watch.stop) => {}
        }
    }
}

/// The module `name` as it is on disk now.
async fn rediscover(context: &Context, name: &str) -> Option<Discovered> {
    let dirs = context.dirs.clone();
    let found = tokio::task::spawn_blocking(move || discover(&dirs))
        .await
        .ok()?;
    found.modules.get(name).cloned()
}

/// How a run that failed ended.
struct Failure {
    exit: Exit,
    stderr_tail: Option<String>,
    error: Option<String>,
}

impl Failure {
    fn because(error: String) -> Self {
        Self {
            exit: Exit::default(),
            stderr_tail: None,
            error: Some(error),
        }
    }

    /// In one sentence.
    fn describe(&self) -> String {
        if let Some(error) = self.error.as_deref().or(self.exit.error.as_deref()) {
            return error.to_owned();
        }
        match (self.exit.code, self.exit.signal) {
            (_, Some(signal)) => format!("it was killed by signal {signal}"),
            (Some(code), None) => format!("it exited with code {code}"),
            (None, None) => "it stopped unexpectedly".into(),
        }
    }
}

/// How one run went.
enum Run {
    Stopped(String),
    /// It isn't what its manifest says, or can't run at all.
    Refused(Failure),
    Failed(Failure),
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
        Err(LaunchError::Impossible(reason)) => return Run::Refused(Failure::because(reason)),
        Err(LaunchError::Failed(reason)) => return Run::Failed(Failure::because(reason)),
    };
    let started = Session::start(
        manifest.clone(),
        launched,
        context.services.clone(),
        &context.timings,
    )
    .await;
    let mut session = match started {
        Ok(session) => session,
        Err((error, mut running)) => {
            let exit = kill(&mut running, &context.timings).await;
            let stderr_tail = running.stderr_tail().await;
            let (refused, reason) = match error {
                StartError::Refused(reason) => (true, reason),
                StartError::Failed(reason) => (false, reason),
            };
            let failure = Failure {
                exit,
                stderr_tail,
                error: Some(reason),
            };
            return if refused {
                Run::Refused(failure)
            } else {
                Run::Failed(failure)
            };
        }
    };
    let pid = session.pid();
    let started = ModuleStarted {
        module: name.to_owned(),
        version: manifest.module.version.clone(),
        pid,
    };
    record(context, name, &started).await;
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
            Run::Stopped(watch.stop_reason().unwrap_or_else(|| HOST_GONE.to_owned()))
        }
        Outcome::Failed(error) => {
            let (exit, stderr_tail) = session.end(&context.timings).await;
            Run::Failed(Failure {
                exit,
                stderr_tail,
                error,
            })
        }
    }
}

async fn stopped(context: &Context, watch: &Watch, name: &str, reason: &str) {
    let stopped = ModuleStopped {
        module: name.to_owned(),
        reason: reason.to_owned(),
    };
    record(context, name, &stopped).await;
    info!(module = %name, %reason, "module stopped");
    watch.set(|s| {
        s.state = State::Stopped;
        s.pid = None;
    });
}

async fn give_up(context: &Context, watch: &Watch, name: &str, restarts: u32, failure: Failure) {
    let described = failure.describe();
    warn!(module = %name, "gave up on a module: {described}");
    record_crash(context, name, restarts, failure, true).await;
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
    failure: Failure,
    gave_up: bool,
) {
    let crashed = ModuleCrashed {
        module: name.to_owned(),
        exit_code: failure.exit.code,
        signal: failure.exit.signal,
        restarts,
        error: failure.error.or(failure.exit.error),
        stderr_tail: failure.stderr_tail,
        gave_up,
    };
    record(context, name, &crashed).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-02
    // suite: unit
    #[test]
    fn failures_are_described_in_one_sentence() {
        let exit = |code, signal| Failure {
            exit: Exit {
                code,
                signal,
                error: None,
            },
            stderr_tail: None,
            error: None,
        };
        assert_eq!(exit(Some(3), None).describe(), "it exited with code 3");
        assert_eq!(exit(None, Some(9)).describe(), "it was killed by signal 9");
        assert_eq!(exit(None, None).describe(), "it stopped unexpectedly");
        assert_eq!(
            Failure::because("its folder is gone".into()).describe(),
            "its folder is gone"
        );
        let builtin = Failure {
            exit: Exit {
                error: Some("it panicked".into()),
                ..Exit::default()
            },
            stderr_tail: None,
            error: None,
        };
        assert_eq!(builtin.describe(), "it panicked");
    }
}
