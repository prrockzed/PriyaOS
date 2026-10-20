//! Modules in the daemon (0.8): the `v1.module.*` methods, and
//! [`DaemonServices`], which gives the module host the event store, the
//! settings and the public API.

use std::sync::{Arc, Weak};

use priya_api::error::{ApiError, ErrorKind};
use priya_api::events::NewEvent;
use priya_api::methods::Done;
use priya_api::methods::events::StoredEvent;
use priya_api::methods::module::{
    Call, Disable, Enable, List, ListResult, ModuleName, ModuleState, ModuleSummary,
};
use priya_api::peer::BoxFuture;
use priya_core::settings::KeyPath;
use priya_store::Filter;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::warn;

use crate::api::{Daemon, newest_in};
use crate::modules::host_api::HostServices;
use crate::modules::{ModuleReport, Presence, ReportState};
use crate::rpc::Router;

/// Events waiting for a module before the store subscription waits.
const DELIVERY_QUEUE: usize = 16;

/// The module host's view of the daemon. It holds the daemon weakly: the
/// daemon owns the host.
pub struct DaemonServices {
    pub daemon: Weak<Daemon>,
    pub router: Arc<Router<Daemon>>,
}

impl DaemonServices {
    fn daemon(&self) -> Result<Arc<Daemon>, ApiError> {
        self.daemon
            .upgrade()
            .ok_or_else(|| ApiError::new(ErrorKind::Unavailable, "priyad is stopping."))
    }
}

impl HostServices for DaemonServices {
    fn record(&self, event: NewEvent) -> BoxFuture<Result<i64, ApiError>> {
        let daemon = self.daemon();
        Box::pin(async move {
            let record = daemon?
                .store
                .append(event)
                .await
                .map_err(|e| ApiError::from_user_facing(&e))?;
            Ok(record.seq)
        })
    }

    fn setting(&self, key: &KeyPath) -> Value {
        let Ok(daemon) = self.daemon() else {
            return Value::Null;
        };
        let all = serde_json::to_value(&*daemon.settings.handle().current()).unwrap_or_default();
        key.parts()
            .iter()
            .try_fold(&all, |node, part| node.get(part))
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn call_api(&self, method: String, params: Value) -> BoxFuture<Result<Value, ApiError>> {
        let daemon = self.daemon();
        let router = self.router.clone();
        Box::pin(async move {
            router
                .call(&daemon?, &method, params)
                .await
                .map_err(|e| e.data)
        })
    }

    fn subscribe(&self, patterns: Vec<String>) -> mpsc::Receiver<StoredEvent> {
        let (tx, rx) = mpsc::channel(DELIVERY_QUEUE);
        // Only the store: the task mustn't keep the daemon alive, and it
        // ends as soon as the module's side goes away.
        let Ok(store) = self.daemon().map(|d| d.store.clone()) else {
            return rx;
        };
        tokio::spawn(async move {
            // From now on: after the newest event.
            let Ok(after) = newest_in(&store).await else {
                return;
            };
            let filter = Filter {
                types: patterns,
                stream_prefix: None,
            };
            let mut records = Box::pin(store.subscribe(filter, after));
            loop {
                let record = tokio::select! {
                    () = tx.closed() => break,
                    record = records.next() => record,
                };
                let Some(Ok(record)) = record else { break };
                // A module gets the full payload, not a link to its blob.
                let mut event = StoredEvent::from(record.clone());
                match store.payload(&record).await {
                    Ok(payload) => event.event.payload = payload,
                    Err(e) => {
                        warn!(error = %e, seq = record.seq, "an event for a module couldn't be read");
                        continue;
                    }
                }
                if tx.send(event).await.is_err() {
                    break;
                }
            }
        });
        rx
    }
}

/// Adds the `v1.module.*` methods.
pub fn add_methods(router: &mut Router<Daemon>) {
    router
        .add::<List, _, _>(|daemon, _, _| async move { Ok(list(&daemon).await) })
        .add::<Enable, _, _>(|daemon, params, _| async move { enable(&daemon, params).await })
        .add::<Disable, _, _>(|daemon, params, _| async move { disable(&daemon, params).await })
        .add::<Call, _, _>(|daemon, params, _| async move {
            daemon.modules.call(&params.method, params.params).await
        });
}

async fn list(daemon: &Daemon) -> ListResult {
    ListResult {
        modules: daemon
            .modules
            .list()
            .await
            .into_iter()
            .map(summary)
            .collect(),
    }
}

fn summary(report: ModuleReport) -> ModuleSummary {
    ModuleSummary {
        name: report.name,
        version: report.version,
        kind: report.kind,
        description: report.description,
        methods: report.methods,
        enabled: report.enabled,
        state: match report.state {
            ReportState::Starting => ModuleState::Starting,
            ReportState::Running => ModuleState::Running,
            ReportState::Restarting => ModuleState::Restarting,
            ReportState::Stopping => ModuleState::Stopping,
            ReportState::Stopped => ModuleState::Stopped,
            ReportState::Failed => ModuleState::Failed,
            ReportState::Disabled => ModuleState::Disabled,
            ReportState::Broken => ModuleState::Broken,
            ReportState::Missing => ModuleState::Missing,
        },
        pid: report.pid,
        restarts: report.restarts,
        problem: report.problem.map(|p| sentence(&p)),
    }
}

/// `it exited with code 3` → `It exited with code 3.`
fn sentence(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut chars = text.chars();
    let mut out: String = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

fn enabled_key() -> KeyPath {
    KeyPath::from_parts(["modules", "enabled"])
}

fn no_such_module(name: &str) -> ApiError {
    ApiError::new(
        ErrorKind::NotFound,
        format!("There's no module named `{name}`. `priya module list` shows the installed ones."),
    )
}

async fn enable(daemon: &Daemon, params: ModuleName) -> Result<Done, ApiError> {
    let name = params.name;
    match daemon.modules.presence(&name).await {
        Presence::Usable => {}
        Presence::Broken(problem) => {
            return Err(ApiError::new(
                ErrorKind::InvalidInput,
                format!("The {name} module can't be used: {}", sentence(&problem)),
            ));
        }
        Presence::Absent => return Err(no_such_module(&name)),
    }
    // Read, change and save the list in one go, so two at once don't lose
    // one of the changes.
    let _changing = daemon.module_changes.lock().await;
    let mut enabled = daemon.settings.handle().current().modules.enabled.clone();
    if !enabled.contains(&name) {
        enabled.push(name.clone());
        save(daemon, &enabled).await?;
    }
    // Enabling one the host gave up on starts it afresh.
    daemon.modules.enable(&enabled, &name).await;
    Ok(Done {})
}

async fn disable(daemon: &Daemon, params: ModuleName) -> Result<Done, ApiError> {
    let name = params.name;
    let _changing = daemon.module_changes.lock().await;
    let mut enabled = daemon.settings.handle().current().modules.enabled.clone();
    if enabled.contains(&name) {
        enabled.retain(|n| *n != name);
        save(daemon, &enabled).await?;
    } else if daemon.modules.presence(&name).await == Presence::Absent {
        return Err(no_such_module(&name));
    }
    daemon.modules.set_enabled(&enabled).await;
    Ok(Done {})
}

async fn save(daemon: &Daemon, enabled: &[String]) -> Result<(), ApiError> {
    daemon
        .settings
        .set(&enabled_key(), &Value::from(enabled.to_vec()))
        .await
        .map_err(|e| ApiError::from_user_facing(&e))
}

#[cfg(test)]
mod tests {
    use priya_api::methods::Method;

    use super::*;

    // covers: CORE-02
    // suite: unit
    #[test]
    fn problems_read_as_sentences() {
        assert_eq!(sentence("it exited with code 3"), "It exited with code 3.");
        assert_eq!(
            sentence("No module with this name is installed."),
            "No module with this name is installed."
        );
        assert_eq!(sentence(""), "");
        assert_eq!(enabled_key().to_string(), "modules.enabled");
        assert_eq!(Enable::NAME, "v1.module.enable");
    }
}
