//! The Phase 0 API methods (02-core §4) on top of the daemon's parts.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use priya_api::error::{ApiError, ErrorKind};
use priya_api::methods::events::{
    EVENT_NOTIFICATION, EventNotification, StoredEvent, Subscription,
};
use priya_api::methods::settings::{GetResult, SchemaResult, SettingLayer, ValueOrigin};
use priya_api::methods::system::{ConfiguredModel, MethodDescription, MethodsResult, StatusResult};
use priya_api::methods::{ALL, Done, audit, events, settings, system};
use priya_core::settings::{KeyPath, Settings, Strategy};
use priya_store::{ChainBreak, EventStore, Filter};
use tokio_stream::StreamExt;
use tracing::warn;

use crate::rpc::{Router, Session};
use crate::settings::SettingsService;

/// What the methods work on.
pub struct Daemon {
    pub store: EventStore,
    pub settings: Arc<SettingsService<EventStore>>,
    pub db_path: PathBuf,
    pub started: Instant,
}

/// Every Phase 0 method, served by the daemon.
pub fn router() -> Router<Daemon> {
    let mut router = Router::default();
    router
        .add::<system::Status, _, _>(|daemon, _, _| async move { status(&daemon).await })
        .add::<system::Methods, _, _>(|_, _, _| async { Ok(methods()) })
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
    router
}

async fn status(daemon: &Daemon) -> Result<StatusResult, ApiError> {
    let event_count: i64 = daemon
        .store
        .query(|conn| Ok(conn.query_row("SELECT count(*) FROM events", [], |r| r.get(0))?))
        .await
        .map_err(|e| ApiError::from_user_facing(&e))?;
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

fn methods() -> MethodsResult {
    MethodsResult {
        methods: ALL
            .iter()
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

async fn get_settings(daemon: &Daemon, params: settings::GetParams) -> Result<GetResult, ApiError> {
    let key = match &params.key {
        Some(text) => key(text)?,
        None => KeyPath::default(),
    };
    let all = serde_json::to_value(&*daemon.settings.handle().current()).map_err(|e| {
        ApiError::new(ErrorKind::Internal, "Settings couldn't be read.").with_detail(e.to_string())
    })?;
    let value = key
        .parts()
        .iter()
        .try_fold(&all, |node, part| node.get(part))
        .cloned()
        .ok_or_else(|| ApiError::new(ErrorKind::NotFound, format!("`{key}` isn't a setting.")))?;
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
    let after = match params.from_seq {
        Some(seq) => seq,
        // From now on: after the newest event.
        None => daemon
            .store
            .query(|conn| {
                Ok(conn.query_row("SELECT coalesce(max(seq), 0) FROM events", [], |r| r.get(0))?)
            })
            .await
            .map_err(|e| ApiError::from_user_facing(&e))?,
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
                        break;
                    }
                };
                // Send the full payload, not a link to its blob.
                let mut event = StoredEvent::from(record.clone());
                match store.payload(&record).await {
                    Ok(payload) => event.event.payload = payload,
                    Err(e) => {
                        warn!(error = %e, seq = record.seq, "sending an event without its blob")
                    }
                }
                let notification = EventNotification {
                    subscription: id.clone(),
                    event,
                };
                if !session.notify(EVENT_NOTIFICATION, &notification).await {
                    break;
                }
            }
        }
    });
    session.hold(id.clone(), task.abort_handle());
    Ok(Subscription { subscription: id })
}

async fn verify(
    daemon: &Daemon,
    params: audit::VerifyParams,
) -> Result<audit::VerifyReport, ApiError> {
    let range = params.from.unwrap_or(1)..=params.to.unwrap_or(i64::MAX);
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
