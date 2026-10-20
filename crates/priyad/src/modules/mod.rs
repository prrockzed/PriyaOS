//! The module host (02-core §5): finds modules, starts the enabled ones,
//! decides what they may do, and restarts them when they fail.
//!
//! - [`discovery`]: which modules exist.
//! - [`launch`]: starting one, as a process or a builtin.
//! - [`session`]: one run, from handshake to exit.
//! - [`supervisor`]: one task per module that restarts it, or gives up.
//! - [`host_api`] and [`access`]: what a module may ask of priyad.
//! - [`ModuleHost`]: all of it, for the daemon and the API.

pub mod access;
pub mod discovery;
pub mod host_api;
pub mod launch;
pub mod policy;
pub mod session;
pub mod supervisor;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use priya_api::error::{ApiError, ErrorKind};
use priya_api::module::{ModuleKind, ModuleManifest};
use priya_api::peer::CallError;
use serde_json::Value;
use tokio::sync::{Mutex, watch};
use tracing::warn;

use self::discovery::{Found, discover};
use self::host_api::HostServices;
use self::launch::{Builtins, Spawn};
use self::policy::RestartPolicy;
use self::session::seconds;
use self::supervisor::{State, Supervisor};

/// How long the host waits for modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    /// For the answer to `initialize`.
    pub handshake: Duration,
    /// Between health pings.
    pub ping_every: Duration,
    /// For the answer to a ping.
    pub ping_timeout: Duration,
    /// For each step of stopping: `shutdown`, SIGTERM, SIGKILL.
    pub stop_grace: Duration,
    /// For a module's answer to `v1.module.call`.
    pub call_timeout: Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(5),
            ping_every: Duration::from_secs(30),
            ping_timeout: Duration::from_secs(5),
            stop_grace: Duration::from_secs(3),
            call_timeout: Duration::from_secs(30),
        }
    }
}

/// Reads `PRIYA_MODULE_TIMINGS`, which tests use to make modules fail fast:
/// `name=milliseconds` pairs separated by commas, such as
/// `ping_every=200,ping_timeout=100`. Names: `handshake`, `ping_every`,
/// `ping_timeout`, `stop_grace`, `call_timeout`, `restart_first`,
/// `restart_max`, `restart_window`, and `max_failures` (a count).
pub fn parse_timings(text: &str) -> Result<(Timings, RestartPolicy), String> {
    let mut timings = Timings::default();
    let mut policy = RestartPolicy::default();
    for pair in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (name, value) = pair
            .split_once('=')
            .ok_or_else(|| format!("`{pair}` isn't name=value"))?;
        let number: u64 = value
            .trim()
            .parse()
            .map_err(|_| format!("`{pair}`: the value must be a whole number"))?;
        let millis = Duration::from_millis(number);
        match name.trim() {
            "handshake" => timings.handshake = millis,
            "ping_every" => timings.ping_every = millis,
            "ping_timeout" => timings.ping_timeout = millis,
            "stop_grace" => timings.stop_grace = millis,
            "call_timeout" => timings.call_timeout = millis,
            "restart_first" => policy.first = millis,
            "restart_max" => policy.max = millis,
            "restart_window" => policy.window = millis,
            "max_failures" => {
                policy.max_failures = usize::try_from(number).map_err(|e| e.to_string())?;
            }
            other => return Err(format!("`{other}` isn't a module timing")),
        }
    }
    if timings.ping_every.is_zero() {
        return Err("`ping_every` must be more than 0".into());
    }
    Ok((timings, policy))
}

/// How the host is set up.
pub struct HostConfig {
    /// Where to look for modules, later folders winning.
    pub dirs: Vec<PathBuf>,
    pub builtins: Builtins,
    pub timings: Timings,
    pub policy: RestartPolicy,
    pub spawn: Spawn,
}

/// What every supervisor shares.
pub struct Context {
    services: Arc<dyn HostServices>,
    builtins: Builtins,
    timings: Timings,
    policy: RestartPolicy,
    spawn: Spawn,
    /// Where modules are found, to read one again before a restart.
    dirs: Vec<PathBuf>,
}

/// What can be told about one module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleReport {
    pub name: String,
    pub version: Option<String>,
    pub kind: Option<ModuleKind>,
    pub description: Option<String>,
    pub methods: Vec<String>,
    pub enabled: bool,
    pub state: ReportState,
    pub pid: Option<u32>,
    pub restarts: u32,
    /// What went wrong last, or why it can't be used.
    pub problem: Option<String>,
}

/// A module's state as reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportState {
    Starting,
    Running,
    Restarting,
    Stopping,
    Stopped,
    /// The host gave up on it.
    Failed,
    /// Found, not enabled.
    Disabled,
    /// Its folder is there but it can't be used.
    Broken,
    /// Enabled, but no such module was found.
    Missing,
}

/// The module host.
pub struct ModuleHost {
    context: Arc<Context>,
    state: Mutex<HostState>,
}

#[derive(Default)]
struct HostState {
    found: Found,
    enabled: Vec<String>,
    supervisors: BTreeMap<String, Supervisor>,
    /// Copies being stopped; each says `true` once it has.
    stopping: BTreeMap<String, watch::Receiver<bool>>,
    /// Set by [`ModuleHost::stop`]: nothing starts any more.
    closed: bool,
}

impl HostState {
    /// Starts a supervisor for `name` if it was found, after any copy of
    /// it that is still stopping.
    fn start(&mut self, context: &Arc<Context>, name: &str) {
        let Some(module) = self.found.modules.get(name).cloned() else {
            return;
        };
        let after = self.stopping.get(name).cloned();
        let supervisor = Supervisor::start(context.clone(), module, after);
        self.supervisors.insert(name.to_owned(), supervisor);
    }

    /// Stops `supervisor` in the background, remembering it until done.
    fn stop(
        &mut self,
        name: String,
        supervisor: Supervisor,
        reason: &str,
    ) -> watch::Receiver<bool> {
        let (done, stopped) = watch::channel(false);
        let reason = reason.to_owned();
        tokio::spawn(async move {
            supervisor.stop(&reason).await;
            done.send_replace(true);
        });
        self.stopping.retain(|_, s| !*s.borrow());
        self.stopping.insert(name, stopped.clone());
        stopped
    }
}

impl ModuleHost {
    pub fn new(config: HostConfig, services: Arc<dyn HostServices>) -> Self {
        Self {
            context: Arc::new(Context {
                services,
                builtins: config.builtins,
                timings: config.timings,
                policy: config.policy,
                spawn: config.spawn,
                dirs: config.dirs,
            }),
            state: Mutex::new(HostState::default()),
        }
    }

    /// Looks for modules again, then brings the running ones in line with
    /// `enabled`: starts the enabled ones that aren't running and stops the
    /// rest, returning once those have stopped. A module the host gave up
    /// on stays given up on until [`retry`](Self::retry) is asked.
    pub async fn set_enabled(&self, enabled: &[String]) {
        let stopping = {
            let mut state = self.state.lock().await;
            if state.closed {
                return;
            }
            state.found = self.discover().await;
            for problem in state.found.problems.iter().filter(|p| p.module.is_none()) {
                warn!(path = %problem.path.display(), "modules folder {}", problem.message);
            }
            state.enabled = enabled.to_vec();
            let leaving: Vec<String> = state
                .supervisors
                .keys()
                .filter(|name| !enabled.contains(name))
                .cloned()
                .collect();
            let mut stopping = Vec::new();
            for name in leaving {
                if let Some(supervisor) = state.supervisors.remove(&name) {
                    stopping.push(state.stop(name, supervisor, "disabled"));
                }
            }
            for name in enabled {
                if !state.supervisors.contains_key(name) {
                    state.start(&self.context, name);
                }
            }
            stopping
        };
        wait_all(stopping).await;
    }

    /// Starts `name` afresh if the host gave up on it (or stopped looking
    /// after it) and it is enabled; enabling a module again does this.
    pub async fn retry(&self, name: &str) {
        let mut state = self.state.lock().await;
        let done = state
            .supervisors
            .get(name)
            .is_some_and(|s| s.status().state == State::Failed);
        if state.closed || !done {
            return;
        }
        state.supervisors.remove(name);
        state.found = self.discover().await;
        state.start(&self.context, name);
    }

    /// Whether a module of this name can be found (looking again).
    pub async fn exists(&self, name: &str) -> bool {
        let mut state = self.state.lock().await;
        state.found = self.discover().await;
        state.found.modules.contains_key(name)
    }

    /// Calls a module's method, `<module>.<method>`, and waits for the
    /// answer.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        let name = method.split('.').next().unwrap_or_default();
        let peer = {
            let state = self.state.lock().await;
            // The running copy's methods count; else what is on disk.
            let methods = match state.supervisors.get(name) {
                Some(supervisor) => supervisor.manifest().provides.methods.clone(),
                None => state
                    .found
                    .modules
                    .get(name)
                    .map(|m| m.manifest.provides.methods.clone())
                    .unwrap_or_default(),
            };
            if !methods.iter().any(|m| m == method) {
                return Err(ApiError::new(
                    ErrorKind::NotFound,
                    format!(
                        "No module offers `{method}`. `priya module list` shows what modules offer."
                    ),
                ));
            }
            state.supervisors.get(name).and_then(Supervisor::peer)
        };
        let Some(peer) = peer else {
            return Err(not_running(name));
        };
        let timeout = self.context.timings.call_timeout;
        let answer = tokio::time::timeout(timeout, peer.call_raw(method, params)).await;
        call_outcome(name, timeout, answer)
    }

    /// Every module found (looking again), enabled or not, every one
    /// running, and every enabled one missing.
    pub async fn list(&self) -> Vec<ModuleReport> {
        let mut state = self.state.lock().await;
        state.found = self.discover().await;
        let mut reports: BTreeMap<String, ModuleReport> = BTreeMap::new();
        for (name, module) in &state.found.modules {
            reports.insert(name.clone(), report(name, &module.manifest, &state.enabled));
        }
        // A running copy wins over what is on disk now.
        for (name, supervisor) in &state.supervisors {
            let mut report = report(name, &supervisor.manifest(), &state.enabled);
            let status = supervisor.status();
            report.state = match status.state {
                State::Starting => ReportState::Starting,
                State::Running => ReportState::Running,
                State::Restarting => ReportState::Restarting,
                State::Stopping => ReportState::Stopping,
                State::Stopped => ReportState::Stopped,
                State::Failed => ReportState::Failed,
            };
            report.pid = status.pid;
            report.restarts = status.restarts;
            report.problem = status.last_error;
            reports.insert(name.clone(), report);
        }
        for problem in &state.found.problems {
            let Some(name) = &problem.module else {
                continue;
            };
            reports.entry(name.clone()).or_insert_with(|| {
                blank(
                    name,
                    &state.enabled,
                    ReportState::Broken,
                    Some(problem.message.clone()),
                )
            });
        }
        for name in &state.enabled {
            let problem = Some("No module with this name is installed.".to_owned());
            reports
                .entry(name.clone())
                .or_insert_with(|| blank(name, &state.enabled, ReportState::Missing, problem));
        }
        reports.into_values().collect()
    }

    /// The modules running now, by name.
    pub async fn running(&self) -> Vec<String> {
        let state = self.state.lock().await;
        state
            .supervisors
            .iter()
            .filter(|(_, s)| s.status().state == State::Running)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Stops every module, recording `reason`, and waits for those already
    /// stopping; nothing starts afterwards. For priyad's own shutdown.
    pub async fn stop(&self, reason: &str) {
        let stopping = {
            let mut state = self.state.lock().await;
            state.closed = true;
            let all = std::mem::take(&mut state.supervisors);
            for (name, supervisor) in all {
                state.stop(name, supervisor, reason);
            }
            state.stopping.values().cloned().collect::<Vec<_>>()
        };
        wait_all(stopping).await;
    }

    /// Looks for modules, and hands their settings schemas on.
    async fn discover(&self) -> Found {
        let dirs = self.context.dirs.clone();
        let found = tokio::task::spawn_blocking(move || discover(&dirs))
            .await
            .unwrap_or_default();
        let schemas = found
            .modules
            .iter()
            .filter_map(|(name, m)| Some((name.clone(), m.settings_schema.clone()?)))
            .collect();
        self.context.services.module_schemas(schemas).await;
        found
    }
}

/// A call's answer, or the error a client should see.
fn call_outcome(
    name: &str,
    timeout: Duration,
    answer: Result<Result<Value, CallError>, tokio::time::error::Elapsed>,
) -> Result<Value, ApiError> {
    match answer {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(CallError::Rpc(error))) => Err(error.data),
        Ok(Err(CallError::Closed)) => Err(ApiError::new(
            ErrorKind::Unavailable,
            format!("The {name} module stopped before it answered."),
        )),
        Ok(Err(error @ CallError::TooLarge(_))) => Err(ApiError::new(
            ErrorKind::InvalidInput,
            "That is too much to send to a module.",
        )
        .with_detail(error.to_string())),
        Ok(Err(error @ CallError::Mismatch(_))) => Err(ApiError::new(
            ErrorKind::Internal,
            "The module's answer couldn't be read.",
        )
        .with_detail(error.to_string())),
        Err(_) => Err(ApiError::new(
            ErrorKind::Timeout,
            format!(
                "The {name} module didn't answer within {}.",
                seconds(timeout)
            ),
        )),
    }
}

/// A module as its manifest describes it, not running.
fn report(name: &str, manifest: &ModuleManifest, enabled: &[String]) -> ModuleReport {
    ModuleReport {
        name: name.to_owned(),
        version: Some(manifest.module.version.clone()),
        kind: Some(manifest.module.kind),
        description: manifest.module.description.clone(),
        methods: manifest.provides.methods.clone(),
        enabled: enabled.iter().any(|e| e == name),
        state: ReportState::Disabled,
        pid: None,
        restarts: 0,
        problem: None,
    }
}

/// A report for a module that isn't running and has no manifest to show.
fn blank(
    name: &str,
    enabled: &[String],
    state: ReportState,
    problem: Option<String>,
) -> ModuleReport {
    ModuleReport {
        name: name.to_owned(),
        version: None,
        kind: None,
        description: None,
        methods: Vec::new(),
        enabled: enabled.iter().any(|e| e == name),
        state,
        pid: None,
        restarts: 0,
        problem,
    }
}

fn not_running(name: &str) -> ApiError {
    ApiError::new(
        ErrorKind::Unavailable,
        format!("The {name} module isn't running. `priya module list` says why."),
    )
}

/// Waits until every one of these has stopped.
async fn wait_all(stopping: Vec<watch::Receiver<bool>>) {
    for mut stopped in stopping {
        let _ = stopped.wait_for(|done| *done).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-02
    // suite: unit
    #[test]
    fn module_timings_can_be_shortened_for_tests() {
        assert_eq!(
            parse_timings(""),
            Ok((Timings::default(), RestartPolicy::default()))
        );
        let (timings, policy) =
            parse_timings("ping_every=200, ping_timeout=100,handshake=900,stop_grace=50,call_timeout=70,restart_first=5,restart_max=40,restart_window=1000,max_failures=2").unwrap();
        assert_eq!(timings.ping_every, Duration::from_millis(200));
        assert_eq!(timings.ping_timeout, Duration::from_millis(100));
        assert_eq!(timings.handshake, Duration::from_millis(900));
        assert_eq!(timings.stop_grace, Duration::from_millis(50));
        assert_eq!(timings.call_timeout, Duration::from_millis(70));
        assert_eq!(policy.first, Duration::from_millis(5));
        assert_eq!(policy.max, Duration::from_millis(40));
        assert_eq!(policy.window, Duration::from_secs(1));
        assert_eq!(policy.max_failures, 2);
        for (bad, why) in [
            ("ping", "isn't name=value"),
            ("ping_every=soon", "whole number"),
            ("nap=1", "isn't a module timing"),
            ("ping_every=0", "more than 0"),
        ] {
            assert!(parse_timings(bad).unwrap_err().contains(why), "{bad}");
        }
    }
}
