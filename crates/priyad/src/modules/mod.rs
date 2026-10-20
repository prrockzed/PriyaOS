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
use priya_api::module::ModuleKind;
use priya_api::peer::CallError;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

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
    dirs: Vec<PathBuf>,
    state: Mutex<HostState>,
}

#[derive(Default)]
struct HostState {
    found: Found,
    enabled: Vec<String>,
    supervisors: BTreeMap<String, Supervisor>,
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
            }),
            dirs: config.dirs,
            state: Mutex::new(HostState::default()),
        }
    }

    /// Looks for modules again, then brings the running ones in line with
    /// `enabled`: starts the enabled ones that aren't running and stops the
    /// rest. A module the host gave up on stays given up on until
    /// [`retry`](Self::retry) is asked.
    pub async fn set_enabled(&self, enabled: &[String]) {
        let leaving = {
            let mut state = self.state.lock().await;
            state.found = self.discover().await;
            state.enabled = enabled.to_vec();
            let leaving: Vec<String> = state
                .supervisors
                .keys()
                .filter(|name| !enabled.contains(name))
                .cloned()
                .collect();
            let leaving: Vec<Supervisor> = leaving
                .iter()
                .filter_map(|name| state.supervisors.remove(name))
                .collect();
            for name in enabled {
                if state.supervisors.contains_key(name) {
                    continue;
                }
                if let Some(module) = state.found.modules.get(name).cloned() {
                    let supervisor = Supervisor::start(self.context.clone(), module);
                    state.supervisors.insert(name.clone(), supervisor);
                }
            }
            leaving
        };
        stop_all(leaving, "disabled").await;
    }

    /// Starts `name` afresh if the host gave up on it and it is enabled
    /// (enabling a module again does this).
    pub async fn retry(&self, name: &str) {
        let mut state = self.state.lock().await;
        let gave_up = state
            .supervisors
            .get(name)
            .is_some_and(|s| s.status().state == State::Failed);
        if !gave_up {
            return;
        }
        state.supervisors.remove(name);
        state.found = self.discover().await;
        if let Some(module) = state.found.modules.get(name).cloned() {
            let supervisor = Supervisor::start(self.context.clone(), module);
            state.supervisors.insert(name.to_owned(), supervisor);
        }
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
            let offered = state
                .found
                .modules
                .get(name)
                .is_some_and(|m| m.manifest.provides.methods.iter().any(|m| m == method));
            if !offered {
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
        match tokio::time::timeout(timeout, peer.call_raw(method, params)).await {
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

    /// Every module found (looking again), enabled or not, and every
    /// enabled one missing.
    pub async fn list(&self) -> Vec<ModuleReport> {
        let mut state = self.state.lock().await;
        state.found = self.discover().await;
        let mut reports: BTreeMap<String, ModuleReport> = BTreeMap::new();
        for (name, module) in &state.found.modules {
            let manifest = &module.manifest;
            let enabled = state.enabled.contains(name);
            let mut report = ModuleReport {
                name: name.clone(),
                version: Some(manifest.module.version.clone()),
                kind: Some(manifest.module.kind),
                description: manifest.module.description.clone(),
                methods: manifest.provides.methods.clone(),
                enabled,
                state: ReportState::Disabled,
                pid: None,
                restarts: 0,
                problem: None,
            };
            if let Some(status) = state.supervisors.get(name).map(Supervisor::status) {
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
            }
            reports.insert(name.clone(), report);
        }
        for problem in &state.found.problems {
            let name = problem
                .path
                .parent()
                .and_then(|folder| folder.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.is_empty() || reports.contains_key(&name) {
                continue;
            }
            reports.insert(
                name.clone(),
                blank(
                    &name,
                    &state.enabled,
                    ReportState::Broken,
                    Some(problem.message.clone()),
                ),
            );
        }
        for name in &state.enabled {
            if !reports.contains_key(name) {
                let problem = Some("No module with this name is installed.".to_owned());
                reports.insert(
                    name.clone(),
                    blank(name, &state.enabled, ReportState::Missing, problem),
                );
            }
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

    /// Stops every module, recording `reason`; for priyad's own shutdown.
    pub async fn stop(&self, reason: &str) {
        let all: Vec<Supervisor> = {
            let mut state = self.state.lock().await;
            std::mem::take(&mut state.supervisors)
                .into_values()
                .collect()
        };
        stop_all(all, reason).await;
    }

    async fn discover(&self) -> Found {
        let dirs = self.dirs.clone();
        tokio::task::spawn_blocking(move || discover(&dirs))
            .await
            .unwrap_or_default()
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

/// Stops supervisors side by side.
async fn stop_all(supervisors: Vec<Supervisor>, reason: &str) {
    let mut stopping = JoinSet::new();
    for supervisor in supervisors {
        let reason = reason.to_owned();
        stopping.spawn(async move { supervisor.stop(&reason).await });
    }
    while stopping.join_next().await.is_some() {}
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
