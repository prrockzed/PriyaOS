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
    /// payloads. The channel closes when priyad stops. Recording must never
    /// wait for a slow subscriber: a module handling an event may itself
    /// record one.
    fn subscribe(&self, patterns: Vec<String>) -> mpsc::Receiver<StoredEvent>;

    /// The settings schemas of the modules found, by module name, so each
    /// module's `[modules.<name>]` section is checked against its own.
    fn module_schemas(&self, schemas: std::collections::BTreeMap<String, Value>) -> BoxFuture<()>;
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
            // Permission first: a module that may not record events learns
            // that, whatever it sent.
            check(manifest, HostCall::EmitEvent).map_err(RpcError::from_api)?;
            let params: EmitEventParams = decode_params(params)?;
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

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;

    /// Records events in a list and answers public calls with their name.
    #[derive(Default)]
    struct Fake {
        events: Mutex<Vec<NewEvent>>,
    }

    impl HostServices for Fake {
        fn record(&self, event: NewEvent) -> BoxFuture<Result<i64, ApiError>> {
            self.events.lock().unwrap().push(event);
            Box::pin(async { Ok(7) })
        }

        fn setting(&self, key: &KeyPath) -> Value {
            json!(key.to_string())
        }

        fn call_api(&self, method: String, params: Value) -> BoxFuture<Result<Value, ApiError>> {
            Box::pin(async move { Ok(json!({"called": method, "with": params})) })
        }

        fn subscribe(&self, _: Vec<String>) -> mpsc::Receiver<StoredEvent> {
            mpsc::channel(1).1
        }

        fn module_schemas(&self, _: std::collections::BTreeMap<String, Value>) -> BoxFuture<()> {
            Box::pin(async {})
        }
    }

    fn api(capabilities: &str) -> (HostApi, Arc<Fake>) {
        let fake = Arc::new(Fake::default());
        let manifest = ModuleManifest::parse(&format!(
            "[module]\nname = \"echo\"\nversion = \"0.1.0\"\napi = \"v1\"\nkind = \"builtin\"\n\
             [requires]\ncapabilities = [{capabilities}]\n"
        ))
        .unwrap();
        let services: Arc<dyn HostServices> = fake.clone();
        (
            HostApi {
                manifest: Arc::new(manifest),
                services,
            },
            fake,
        )
    }

    async fn ask(api: &HostApi, method: &str, params: Value) -> Result<Value, RpcError> {
        api.handle(method.to_owned(), Some(params)).await
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn host_calls_do_what_the_manifest_allows_and_nothing_else() {
        let (open, fake) = api("\"events.emit\", \"api.read\"");
        let seq = ask(
            &open,
            "host.emit_event",
            json!({"custom_type": "forecast.updated", "payload": {"c": 21}}),
        )
        .await;
        assert_eq!(seq.unwrap(), json!({"seq": 7}));
        let event = fake.events.lock().unwrap().remove(0);
        assert_eq!(
            (event.event_type.as_str(), event.stream.as_str()),
            ("module.event", "module:echo")
        );
        assert_eq!(event.actor.to_string(), "module:echo");
        assert_eq!(
            event.payload,
            json!({"module": "echo", "name": "forecast.updated", "data": {"c": 21}})
        );
        let bad_name = ask(
            &open,
            "host.emit_event",
            json!({"custom_type": "Forecast!"}),
        )
        .await
        .unwrap_err();
        assert_eq!(bad_name.data.kind, ErrorKind::InvalidInput);
        assert!(bad_name.data.user_message.contains("can't name an event"));

        let own = ask(
            &open,
            "host.settings.get",
            json!({"key": "modules.echo.prefix"}),
        )
        .await;
        assert_eq!(own.unwrap(), json!({"value": "modules.echo.prefix"}));
        let bad_key = ask(&open, "host.settings.get", json!({"key": "not a key!"}))
            .await
            .unwrap_err();
        assert_eq!(bad_key.data.kind, ErrorKind::InvalidInput);
        let status = ask(
            &open,
            "host.call",
            json!({"method": "v1.system.status", "params": {"x": 1}}),
        )
        .await;
        assert_eq!(
            status.unwrap(),
            json!({"called": "v1.system.status", "with": {"x": 1}})
        );

        let unknown = ask(&open, "host.secret.get", json!({})).await.unwrap_err();
        assert_eq!(unknown.code, -32601);
        // A notification is ignored.
        open.notified("host.emit_event".into(), Some(json!({})))
            .await;
        assert!(fake.events.lock().unwrap().is_empty());

        // Without the capability, permission is refused before the params
        // are even read.
        let (closed, _) = api("");
        let refused = ask(&closed, "host.emit_event", json!({"nonsense": true}))
            .await
            .unwrap_err();
        assert_eq!(refused.data.kind, ErrorKind::PermissionDenied);
        let refused = ask(&closed, "host.call", json!({"method": "v1.system.status"}))
            .await
            .unwrap_err();
        assert_eq!(refused.data.kind, ErrorKind::PermissionDenied);
    }
}
