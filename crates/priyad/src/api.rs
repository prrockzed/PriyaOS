//! The Phase 0 API methods (02-core §4) on top of the daemon's parts.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use priya_api::error::{ApiError, ErrorKind};
use priya_api::methods::events::{
    EVENT_NOTIFICATION, EndReason, EventNotification, StoredEvent, Subscription,
};
use priya_api::methods::settings::{GetResult, SchemaResult, SettingLayer, ValueOrigin};
use priya_api::methods::system::{ConfiguredModel, MethodDescription, MethodsResult, StatusResult};
use priya_api::methods::{ALL, Done, Method, audit, events, settings, system};
use priya_core::settings::{KeyPath, Settings, Strategy, is_setting};
use priya_store::{ChainBreak, EventStore, Filter};
use tokio_stream::StreamExt;
use tracing::warn;

use crate::daemon::RuntimeFiles;
use crate::doctor;
use crate::rpc::{Router, Session};
use crate::settings::SettingsService;

/// What the methods work on.
pub struct Daemon {
    pub store: EventStore,
    pub settings: Arc<SettingsService<EventStore>>,
    pub db_path: PathBuf,
    pub started: Instant,
    pub doctor: DoctorPaths,
}

/// The most subscriptions one connection may hold.
pub const MAX_SUBSCRIPTIONS: usize = 64;
/// The most type patterns one subscription may have.
pub const MAX_PATTERNS: usize = 64;
/// The longest type pattern or stream prefix, in bytes.
pub const MAX_PATTERN_LEN: usize = 256;

/// What `priya doctor` looks at, besides the daemon's own parts.
#[derive(Debug, Clone)]
pub struct DoctorPaths {
    pub runtime: RuntimeFiles,
    pub data_dir: PathBuf,
    /// `/proc/self/mountinfo` and `/etc/fstab`.
    pub mountinfo: PathBuf,
    pub fstab: PathBuf,
}

/// Every Phase 0 method, served by the daemon.
pub fn router() -> Router<Daemon> {
    let mut router = Router::default();
    router
        .add::<system::Status, _, _>(|daemon, _, _| async move { status(&daemon).await })
        .add::<system::Doctor, _, _>(|daemon, params, _| async move {
            let paths = daemon.doctor.clone();
            let context = doctor::Context {
                daemon,
                runtime: paths.runtime,
                data_dir: paths.data_dir,
                cli_version: params.cli_version,
                mountinfo: paths.mountinfo,
                fstab: paths.fstab,
            };
            Ok(doctor::run(&context, &doctor::checks(), params.fix).await)
        })
        .add::<settings::Get, _, _>(|daemon, params, _| async move {
            get_settings(&daemon, params).await
        })
        .add::<settings::Set, _, _>(|daemon, params, _| async move {
            let key = key(&params.key)?;
            daemon
                .settings
                .set(&key, &params.value)
                .await
                .map_err(|e| ApiError::from_user_facing(&e))?;
            Ok(Done {})
        })
        .add::<settings::Unset, _, _>(|daemon, params, _| async move {
            let key = key(&params.key)?;
            if !is_setting(&key) {
                return Err(not_a_setting(&key));
            }
            daemon
                .settings
                .unset(&key)
                .await
                .map_err(|e| ApiError::from_user_facing(&e))?;
            Ok(Done {})
        })
        .add::<settings::GetSchema, _, _>(|_, _, _| async {
            Ok(SchemaResult {
                schema: Settings::json_schema().to_value(),
            })
        })
        .add::<events::Subscribe, _, _>(|daemon, params, session| async move {
            subscribe(&daemon, params, &session).await
        })
        .add::<events::Unsubscribe, _, _>(|_, params, session| async move {
            if session.cancel(&params.subscription) {
                Ok(Done {})
            } else {
                Err(ApiError::new(
                    ErrorKind::NotFound,
                    "There's no such subscription on this connection.",
                ))
            }
        })
        .add::<audit::Verify, _, _>(
            |daemon, params, _| async move { verify(&daemon, params).await },
        );
    // Added last, so it lists exactly what is served.
    let served = methods(router.names().chain([system::Methods::NAME]));
    router.add::<system::Methods, _, _>(move |_, _, _| {
        let served = served.clone();
        async move { Ok(served) }
    });
    router
}

async fn status(daemon: &Daemon) -> Result<StatusResult, ApiError> {
    let event_count = newest_seq(daemon).await?;
    let model = daemon
        .settings
        .handle()
        .current()
        .ai
        .strategy
        .as_ref()
        .map(|strategy| match strategy {
            Strategy::Fixed { provider, model } => ConfiguredModel {
                provider: provider.to_string(),
                model: model.clone(),
            },
        });
    Ok(StatusResult {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        uptime_seconds: daemon.started.elapsed().as_secs(),
        db_path: daemon.db_path.display().to_string(),
        event_count: u64::try_from(event_count).unwrap_or(0),
        modules: Vec::new(),
        model,
    })
}

/// The newest event's seq, which is how many there are: seqs start at 1
/// with no gaps (the chain check finds any).
async fn newest_seq(daemon: &Daemon) -> Result<i64, ApiError> {
    daemon
        .store
        .query(|conn| {
            Ok(conn.query_row("SELECT coalesce(max(seq), 0) FROM events", [], |r| r.get(0))?)
        })
        .await
        .map_err(|e| ApiError::from_user_facing(&e))
}

/// The descriptions of the methods named.
fn methods<'a>(names: impl Iterator<Item = &'a str>) -> MethodsResult {
    let names: Vec<&str> = names.collect();
    MethodsResult {
        methods: ALL
            .iter()
            .filter(|method| names.contains(&method.name))
            .map(|method| MethodDescription {
                name: method.name.to_owned(),
                summary: method.summary.to_owned(),
                read_only: method.read_only,
                notification: method.notification.map(str::to_owned),
                params: (method.params)().to_value(),
                result: (method.result)().to_value(),
            })
            .collect(),
    }
}

/// A dotted settings key from a request.
fn key(text: &str) -> Result<KeyPath, ApiError> {
    KeyPath::parse(text).map_err(|message| ApiError::new(ErrorKind::InvalidInput, message))
}

fn not_a_setting(key: &KeyPath) -> ApiError {
    ApiError::new(
        ErrorKind::NotFound,
        format!("`{key}` isn't a setting. `priya settings schema` lists them all."),
    )
}

async fn get_settings(daemon: &Daemon, params: settings::GetParams) -> Result<GetResult, ApiError> {
    let key = match &params.key {
        Some(text) => key(text)?,
        None => KeyPath::default(),
    };
    let all = serde_json::to_value(&*daemon.settings.handle().current()).map_err(|e| {
        ApiError::new(ErrorKind::Internal, "Settings couldn't be read.").with_detail(e.to_string())
    })?;
    let found = key
        .parts()
        .iter()
        .try_fold(&all, |node, part| node.get(part))
        .cloned();
    let value = match found {
        Some(value) => value,
        // A real setting with no value, such as `ai.strategy` before a
        // model is chosen.
        None if is_setting(&key) => serde_json::Value::Null,
        None => return Err(not_a_setting(&key)),
    };
    let origins = if params.explain {
        let explained = daemon.settings.explain(&key).await;
        Some(
            explained
                .into_iter()
                .map(|e| ValueOrigin {
                    key: e.key.to_string(),
                    value: e.value,
                    layer: e
                        .origin
                        .as_ref()
                        .map_or(SettingLayer::BuiltIn, |o| o.layer.into()),
                    file: e
                        .origin
                        .as_ref()
                        .and_then(|o| o.file.as_ref())
                        .map(|f| f.display().to_string()),
                    line: e.origin.as_ref().and_then(|o| o.line),
                })
                .collect(),
        )
    } else {
        None
    };
    Ok(GetResult { value, origins })
}

async fn subscribe(
    daemon: &Daemon,
    params: events::SubscribeParams,
    session: &Session,
) -> Result<Subscription, ApiError> {
    check_subscription(&params, session)?;
    let after = match params.from_seq {
        Some(seq) => seq,
        // From now on: after the newest event.
        None => newest_seq(daemon).await?,
    };
    let filter = Filter {
        types: params.types,
        stream_prefix: params.stream_prefix,
    };
    let id = session.new_subscription_id();
    let mut stream = Box::pin(daemon.store.subscribe(filter, after));
    let store = daemon.store.clone();
    let replied = session.after_reply();
    let task = tokio::spawn({
        let session = session.clone();
        let id = id.clone();
        async move {
            // The client must have the id before the first event.
            let _ = replied.await;
            while let Some(record) = stream.next().await {
                let record = match record {
                    Ok(record) => record,
                    Err(e) => {
                        warn!(error = %e, "a subscription stopped");
                        let message = "The event history couldn't be read, so this stream stopped.";
                        session.end(&id, EndReason::Error, message).await;
                        return;
                    }
                };
                // Send the full payload, not a link to its blob: an event
                // without its payload would be wrong, so the stream ends.
                let mut event = StoredEvent::from(record.clone());
                match store.payload(&record).await {
                    Ok(payload) => event.event.payload = payload,
                    Err(e) => {
                        warn!(error = %e, seq = record.seq, "an event's payload couldn't be read");
                        let message = format!(
                            "Event {} couldn't be read, so this stream stopped. `priya audit verify` checks the history.",
                            record.seq
                        );
                        session.end(&id, EndReason::Error, &message).await;
                        return;
                    }
                }
                let notification = EventNotification {
                    subscription: id.clone(),
                    event,
                };
                if !session
                    .notify_for(&id, EVENT_NOTIFICATION, &notification)
                    .await
                {
                    return;
                }
            }
            // The store has closed: the daemon is stopping.
            let message = "The event history closed, so this stream stopped.";
            session.end(&id, EndReason::Cancelled, message).await;
        }
    });
    session.hold(id.clone(), task.abort_handle());
    Ok(Subscription { subscription: id })
}

/// Refuses subscriptions that would cost the daemon too much.
fn check_subscription(params: &events::SubscribeParams, session: &Session) -> Result<(), ApiError> {
    let refuse = |message: String| Err(ApiError::new(ErrorKind::InvalidInput, message));
    if session.subscription_count() >= MAX_SUBSCRIPTIONS {
        return refuse(format!(
            "This connection already has {MAX_SUBSCRIPTIONS} subscriptions; stop one first."
        ));
    }
    if params.types.len() > MAX_PATTERNS {
        return refuse(format!("Give at most {MAX_PATTERNS} event types."));
    }
    let too_long = params
        .types
        .iter()
        .chain(&params.stream_prefix)
        .any(|pattern| pattern.len() > MAX_PATTERN_LEN);
    if too_long {
        return refuse(format!(
            "Event types and stream prefixes can be at most {MAX_PATTERN_LEN} bytes long."
        ));
    }
    if params.from_seq.is_some_and(|seq| seq < 0) {
        return refuse("`from_seq` can't be negative.".to_owned());
    }
    Ok(())
}

async fn verify(
    daemon: &Daemon,
    params: audit::VerifyParams,
) -> Result<audit::VerifyReport, ApiError> {
    let from = params.from.unwrap_or(1);
    let to = params.to.unwrap_or(i64::MAX);
    if from < 1 || to < 1 {
        return Err(ApiError::new(
            ErrorKind::InvalidInput,
            "Events are numbered from 1.",
        ));
    }
    if from > to {
        return Err(ApiError::new(
            ErrorKind::InvalidInput,
            format!("The range is backwards: {from} comes after {to}."),
        ));
    }
    let range = from..=to;
    let report = daemon
        .store
        .verify_chain(range)
        .await
        .map_err(|e| ApiError::from_user_facing(&e))?;
    Ok(audit::VerifyReport {
        ok: report.ok,
        checked: report.checked,
        first_bad_seq: report.first_bad_seq,
        reason: report.reason.map(|reason| match reason {
            ChainBreak::HashMismatch => audit::BreakReason::HashMismatch,
            ChainBreak::PrevLinkBroken => audit::BreakReason::PrevLinkBroken,
            ChainBreak::SeqGap => audit::BreakReason::SeqGap,
        }),
    })
}
