//! The host API: what a module may ask of priyad (`host.*`), each call
//! checked against its manifest first (02-core §5.2).
//!
//! The module host doesn't know the store, the settings or the public API
//! directly: it uses them through [`HostServices`], a port the daemon
//! fills in, so the host can be tested on its own.

use std::sync::Arc;

use priya_api::error::{ApiError, ErrorKind};
use priya_api::events::{Actor, ModuleEvent, NewEvent};
use priya_api::methods::decode_params;
use priya_api::methods::events::StoredEvent;
use priya_api::module::ModuleManifest;
use priya_api::module::protocol::{
    CallApiParams, EmitEventParams, EmitEventResult, GetSettingParams, GetSettingResult,
    is_custom_type,
};
use priya_api::peer::{BoxFuture, Handler};
use priya_api::rpc::RpcError;
use priya_core::settings::KeyPath;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use super::access::{HostCall, check};

/// What the module host needs from the rest of priyad.
pub trait HostServices: Send + Sync + 'static {
    /// Records `event`; returns its seq.
    fn record(&self, event: NewEvent) -> BoxFuture<Result<i64, ApiError>>;

    /// The value of a setting in force; null when it isn't set.
    fn setting(&self, key: &KeyPath) -> Value;

    /// Calls a public API method (already checked as allowed).
    fn call_api(&self, method: String, params: Value) -> BoxFuture<Result<Value, ApiError>>;

    /// Events from now on whose type matches one of `patterns`, with full
    /// payloads. The channel closes when priyad stops.
    fn subscribe(&self, patterns: Vec<String>) -> mpsc::Receiver<StoredEvent>;
}

/// Answers one module's `host.*` calls.
pub struct HostApi {
    pub manifest: Arc<ModuleManifest>,
    pub services: Arc<dyn HostServices>,
}

impl Handler for HostApi {
    fn handle(&self, method: String, params: Option<Value>) -> BoxFuture<Result<Value, RpcError>> {
        let (manifest, services) = (self.manifest.clone(), self.services.clone());
        Box::pin(async move {
            let outcome = answer(&manifest, &*services, &method, params).await;
            if let Err(error) = &outcome
                && error.data.kind == ErrorKind::PermissionDenied
            {
                warn!(module = manifest.name(), %method, "refused a host call: {}", error.data.user_message);
            }
            outcome
        })
    }

    /// Modules have nothing to notify priyad of; one is ignored.
    fn notified(&self, method: String, _: Option<Value>) -> BoxFuture<()> {
        debug!(module = self.manifest.name(), %method, "ignored a notification from a module");
        Box::pin(async {})
    }
}

async fn answer(
    manifest: &ModuleManifest,
    services: &dyn HostServices,
    method: &str,
    params: Option<Value>,
) -> Result<Value, RpcError> {
    let name = manifest.name();
    match method {
        "host.emit_event" => {
            let params: EmitEventParams = decode_params(params)?;
            check(manifest, HostCall::EmitEvent).map_err(RpcError::from_api)?;
            if !is_custom_type(&params.custom_type) {
                return Err(RpcError::from_api(ApiError::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "`{}` can't name an event: use lowercase words joined by dots, like `forecast.updated`.",
                        params.custom_type
                    ),
                )));
            }
            let event = NewEvent::new(
                format!("module:{name}"),
                Actor::Module(name.to_owned()),
                &ModuleEvent {
                    module: name.to_owned(),
                    name: params.custom_type,
                    data: params.payload,
                },
            )
            .map_err(|e| RpcError::from_api(ApiError::from_user_facing(&e)))?;
            let seq = services.record(event).await.map_err(RpcError::from_api)?;
            to_value(&EmitEventResult { seq })
        }
        "host.settings.get" => {
            let params: GetSettingParams = decode_params(params)?;
            let key = KeyPath::parse(&params.key).map_err(|message| {
                RpcError::from_api(ApiError::new(ErrorKind::InvalidInput, message))
            })?;
            check(manifest, HostCall::ReadSetting(&key)).map_err(RpcError::from_api)?;
            to_value(&GetSettingResult {
                value: services.setting(&key),
            })
        }
        "host.call" => {
            let params: CallApiParams = decode_params(params)?;
            let call = HostCall::CallApi {
                method: &params.method,
                params: &params.params,
            };
            check(manifest, call).map_err(RpcError::from_api)?;
            services
                .call_api(params.method, params.params)
                .await
                .map_err(RpcError::from_api)
        }
        _ => Err(RpcError::method_not_found(method)),
    }
}

fn to_value(value: &impl serde::Serialize) -> Result<Value, RpcError> {
    serde_json::to_value(value).map_err(|e| RpcError::internal(e.to_string()))
}
