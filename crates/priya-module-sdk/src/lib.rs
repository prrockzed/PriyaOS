//! Write PriyaOS modules in Rust (02-core §5).
//!
//! A module is a type that implements [`Module`]: it says who it is when
//! priyad starts it, then answers its methods. [`HostClient`] is how it
//! talks back to priyad: record events, read settings, call the read-only
//! public API. The SDK answers `ping` and `shutdown` itself.
//!
//! The same module runs two ways, with the same messages:
//! - as its own process, with [`run_stdio`] in `main` (`kind = "process"`);
//! - inside priyad, through [`serve`] over in-process channels
//!   (`kind = "builtin"`).
//!
//! ```no_run
//! use priya_module_sdk::{ApiError, HostClient, InitializeParams, InitializeResult, Module, Value, json};
//!
//! struct Hello;
//!
//! impl Module for Hello {
//!     async fn initialize(&self, _: &HostClient, _: InitializeParams) -> Result<InitializeResult, ApiError> {
//!         Ok(InitializeResult::v1("hello", env!("CARGO_PKG_VERSION"), ["hello.greet"]))
//!     }
//!
//!     async fn handle(&self, host: &HostClient, method: &str, _: Value) -> Result<Value, ApiError> {
//!         match method {
//!             "hello.greet" => {
//!                 host.emit_event("greeted", json!({})).await?;
//!                 Ok(json!({"text": "Hello!"}))
//!             }
//!             _ => Err(priya_module_sdk::no_such_method(method)),
//!         }
//!     }
//! }
//!
//! fn main() -> std::process::ExitCode {
//!     priya_module_sdk::run_stdio(Hello)
//! }
//! ```
//!
//! stdout carries the protocol, so a module must never print to it; what
//! it writes to stderr shows up in priyad's log under its name.

use std::future::Future;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use priya_api::framing::lsp::{read_messages, write_messages};
use priya_api::methods::decode_params;
use priya_api::module::protocol::{
    CallApi, CallApiParams, EVENT_NOTIFICATION, EmitEvent, EmitEventParams, EventDelivery,
    GetSetting, GetSettingParams,
};
use priya_api::peer::{BoxFuture, CallError, Inbound, Peer};
use priya_api::rpc::RpcError;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;

pub use priya_api::error::{ApiError, ErrorKind};
pub use priya_api::methods::events::StoredEvent;
pub use priya_api::module::protocol::{InitializeParams, InitializeResult};
pub use priya_api::peer::Ended;
pub use serde_json::{Value, json};

/// Messages waiting in each direction.
const QUEUE: usize = 64;

/// A PriyaOS module.
pub trait Module: Send + Sync + 'static {
    /// Called once, first. Returns who the module is; priyad refuses it
    /// unless this matches its `module.toml`.
    fn initialize(
        &self,
        host: &HostClient,
        params: InitializeParams,
    ) -> impl Future<Output = Result<InitializeResult, ApiError>> + Send;

    /// Answers one of the module's methods (`<module>.<method>`).
    fn handle(
        &self,
        host: &HostClient,
        method: &str,
        params: Value,
    ) -> impl Future<Output = Result<Value, ApiError>> + Send;

    /// An event the module subscribes to (`[subscribes] events`).
    fn event(&self, host: &HostClient, event: StoredEvent) -> impl Future<Output = ()> + Send {
        let _ = (host, event);
        async {}
    }

    /// Called when priyad asks the module to stop, before it answers.
    fn shutdown(&self) -> impl Future<Output = ()> + Send {
        async {}
    }
}

/// The error for a method the module doesn't have.
pub fn no_such_method(method: &str) -> ApiError {
    ApiError::new(
        ErrorKind::NotFound,
        "This module doesn't know that request.",
    )
    .with_detail(format!("no method named {method}"))
}

/// A method's params as `P`, or an `invalid_input` error saying what is
/// wrong. Absent or null params count as `{}`.
pub fn params<P: DeserializeOwned>(params: Value) -> Result<P, ApiError> {
    decode_params(Some(params)).map_err(|e| e.data)
}

/// The module's line to priyad. Cheap to clone.
#[derive(Clone)]
pub struct HostClient {
    peer: Peer,
}

impl HostClient {
    /// Records one of the module's own events, as `module.event`; returns
    /// its place in the log. Needs `events.emit` in the manifest.
    pub async fn emit_event(&self, custom_type: &str, payload: Value) -> Result<i64, ApiError> {
        let params = EmitEventParams {
            custom_type: custom_type.to_owned(),
            payload,
        };
        let result = self.peer.call::<EmitEvent>(&params).await;
        Ok(result.map_err(host_error)?.seq)
    }

    /// The value of the setting `key` (dotted, such as
    /// `modules.echo.prefix`); null when it isn't set. The module's own
    /// `modules.<name>` section is always readable; other keys need
    /// `settings.read:<prefix>`.
    pub async fn setting(&self, key: &str) -> Result<Value, ApiError> {
        let params = GetSettingParams {
            key: key.to_owned(),
        };
        let result = self.peer.call::<GetSetting>(&params).await;
        Ok(result.map_err(host_error)?.value)
    }

    /// Calls a read-only public API method, such as `v1.system.status`.
    /// Needs `api.read`.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        let params = CallApiParams {
            method: method.to_owned(),
            params,
        };
        self.peer.call::<CallApi>(&params).await.map_err(host_error)
    }
}

/// A failed host call as the module sees it.
fn host_error(error: CallError) -> ApiError {
    match error {
        CallError::Rpc(error) => error.data,
        CallError::Closed => ApiError::new(ErrorKind::Unavailable, "priyad has gone away."),
        CallError::TooLarge(_) => ApiError::new(
            ErrorKind::InvalidInput,
            "That is too much to send to priyad.",
        )
        .with_detail(error.to_string()),
        CallError::Mismatch(_) => {
            ApiError::new(ErrorKind::Internal, "priyad's answer couldn't be read.")
                .with_detail(error.to_string())
        }
    }
}

/// Serves `module` over a pair of channels until priyad says `shutdown`
/// or goes away; returns how the conversation ended. priyad runs builtin
/// modules this way.
pub async fn serve<M: Module>(
    module: M,
    inbound: mpsc::Receiver<Inbound>,
    outbound: mpsc::Sender<Vec<u8>>,
) -> Ended {
    let module = Arc::new(module);
    let (_peer, task) = Peer::spawn(inbound, outbound, move |peer| {
        let dispatch = Arc::new(Dispatch {
            module,
            host: HostClient { peer },
            initialized: AtomicBool::new(false),
        });
        move |method: String, params: Option<Value>| {
            let dispatch = dispatch.clone();
            async move { dispatch.handle(method, params).await }
        }
    });
    task.await
        .unwrap_or_else(|e| Ended::Broken(format!("the module stopped: {e}")))
}

/// What priyad keeps for each builtin module: starts it on a pair of
/// channels. A module offers one as `builtin::<MyModule>`.
pub type BuiltinMain = fn(mpsc::Receiver<Inbound>, mpsc::Sender<Vec<u8>>) -> BoxFuture<Ended>;

/// Serves a fresh `M` on a pair of channels: a [`BuiltinMain`].
pub fn builtin<M: Module + Default>(
    inbound: mpsc::Receiver<Inbound>,
    outbound: mpsc::Sender<Vec<u8>>,
) -> BoxFuture<Ended> {
    Box::pin(serve(M::default(), inbound, outbound))
}

/// Serves `module` on stdin and stdout until priyad says `shutdown` or
/// goes away. Every answer is written out before it returns.
pub async fn serve_stdio<M: Module>(module: M) -> Ended {
    let (to_module, inbound) = mpsc::channel(QUEUE);
    let (outbound, to_host) = mpsc::channel(QUEUE);
    tokio::spawn(read_messages(tokio::io::stdin(), to_module));
    let writer = tokio::spawn(write_messages(tokio::io::stdout(), to_host));
    let ended = serve(module, inbound, outbound).await;
    // The channel closes when the conversation ends, so this finishes once
    // the last answer is out.
    match writer.await {
        Ok(Err(e)) if ended == Ended::Closed => Ended::Broken(format!("writing failed: {e}")),
        _ => ended,
    }
}

/// `main` for a process module: serves `module` on stdio and exits 0 when
/// told to, or 1 (saying why on stderr) if the conversation broke.
pub fn run_stdio<M: Module>(module: M) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            report(&format!("couldn't start: {e}"));
            return ExitCode::FAILURE;
        }
    };
    let ended = runtime.block_on(serve_stdio(module));
    // stdin is read on a blocking thread that can't be interrupted; don't
    // wait for it.
    runtime.shutdown_background();
    match ended {
        Ended::Closed => ExitCode::SUCCESS,
        Ended::Broken(reason) => {
            report(&reason);
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::print_stderr)] // stderr is the module's log
fn report(message: &str) {
    eprintln!("{message}");
}

/// Routes priyad's requests: the protocol's own methods here, the rest to
/// the module.
struct Dispatch<M> {
    module: Arc<M>,
    host: HostClient,
    initialized: AtomicBool,
}

impl<M: Module> Dispatch<M> {
    async fn handle(&self, method: String, params: Option<Value>) -> Result<Value, RpcError> {
        let host = &self.host;
        let params = params.unwrap_or(Value::Null);
        match method.as_str() {
            "initialize" => {
                let params = decode_params(Some(params))?;
                let hello = self
                    .module
                    .initialize(host, params)
                    .await
                    .map_err(RpcError::from_api)?;
                self.initialized.store(true, Ordering::Release);
                to_value(&hello)
            }
            "ping" => Ok(json!({})),
            "shutdown" => {
                self.module.shutdown().await;
                // Stops reading; this answer still goes out.
                host.peer.close();
                Ok(json!({}))
            }
            _ if !self.initialized.load(Ordering::Acquire) => Err(RpcError::from_api(not_ready())),
            EVENT_NOTIFICATION => {
                let delivery: EventDelivery = decode_params(Some(params))?;
                self.module.event(host, delivery.event).await;
                Ok(Value::Null)
            }
            _ => self
                .module
                .handle(host, &method, params)
                .await
                .map_err(RpcError::from_api),
        }
    }
}

fn not_ready() -> ApiError {
    ApiError::new(ErrorKind::Unavailable, "The module is still starting.")
}

fn to_value(value: &impl serde::Serialize) -> Result<Value, RpcError> {
    serde_json::to_value(value).map_err(|e| RpcError::internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use priya_api::module::protocol::{Empty, Initialize, Ping, Shutdown};

    use super::*;

    /// A module that reports what it saw.
    #[derive(Default)]
    struct Recorder {
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl Module for Recorder {
        async fn initialize(
            &self,
            _: &HostClient,
            params: InitializeParams,
        ) -> Result<InitializeResult, ApiError> {
            self.seen
                .lock()
                .unwrap()
                .push(format!("initialize {}", params.host_version));
            Ok(InitializeResult::v1(
                "rec",
                "1.0.0",
                ["rec.ask", "rec.fail"],
            ))
        }

        async fn handle(
            &self,
            host: &HostClient,
            method: &str,
            params: Value,
        ) -> Result<Value, ApiError> {
            match method {
                "rec.ask" => {
                    let seq = host.emit_event("asked", params.clone()).await?;
                    let theme = host.setting("ui.theme").await?;
                    let status = host.call("v1.system.status", json!({})).await?;
                    Ok(json!({"seq": seq, "theme": theme, "status": status}))
                }
                "rec.fail" => Err(host.setting("secret").await.unwrap_err()),
                _ => Err(no_such_method(method)),
            }
        }

        async fn event(&self, _: &HostClient, event: StoredEvent) {
            self.seen
                .lock()
                .unwrap()
                .push(format!("event {}", event.seq));
        }

        async fn shutdown(&self) {
            self.seen.lock().unwrap().push("shutdown".into());
        }
    }

    /// A stand-in for priyad answering the host calls.
    async fn fake_host(method: String, params: Option<Value>) -> Result<Value, RpcError> {
        match method.as_str() {
            "host.emit_event" => Ok(json!({"seq": 7})),
            "host.settings.get" if params.as_ref().is_some_and(|p| p["key"] == "secret") => Err(
                RpcError::from_api(ApiError::new(ErrorKind::PermissionDenied, "Not yours.")),
            ),
            "host.settings.get" => Ok(json!({"value": "dark"})),
            "host.call" => Ok(json!({"version": "0.0.0"})),
            _ => Err(RpcError::method_not_found(&method)),
        }
    }

    /// The module served on channels, and priyad's end of them.
    fn start(
        module: Recorder,
    ) -> (
        Peer,
        tokio::task::JoinHandle<Ended>,
        tokio::task::JoinHandle<Ended>,
    ) {
        let (to_module, module_in) = mpsc::channel(8);
        let (module_out, mut from_module) = mpsc::channel::<Vec<u8>>(8);
        let (host_in_tx, host_in) = mpsc::channel(8);
        tokio::spawn(async move {
            while let Some(body) = from_module.recv().await {
                if host_in_tx.send(Ok(body)).await.is_err() {
                    break;
                }
            }
        });
        let (to_module_bytes, mut module_bytes) = mpsc::channel::<Vec<u8>>(8);
        tokio::spawn(async move {
            while let Some(body) = module_bytes.recv().await {
                if to_module.send(Ok(body)).await.is_err() {
                    break;
                }
            }
        });
        let served = tokio::spawn(serve(module, module_in, module_out));
        let (host, host_task) = Peer::spawn(host_in, to_module_bytes, |_| fake_host);
        (host, served, host_task)
    }

    fn init() -> InitializeParams {
        InitializeParams {
            api: "v1".into(),
            host_version: "9.9.9".into(),
        }
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn a_module_answers_after_initialize_and_stops_on_shutdown() {
        let module = Recorder::default();
        let seen = module.seen.clone();
        let (host, served, _) = start(module);

        // Nothing but the protocol's own methods before initialize.
        let early = host.call_raw("rec.ask", json!({})).await.unwrap_err();
        assert!(matches!(early, CallError::Rpc(e) if e.data.kind == ErrorKind::Unavailable));
        host.call::<Ping>(&Empty {}).await.unwrap();

        let hello = host.call::<Initialize>(&init()).await.unwrap();
        assert_eq!(hello.api, "v1");
        assert_eq!((hello.name.as_str(), hello.methods.len()), ("rec", 2));

        // The module's method calls the host three ways.
        let answer = host.call_raw("rec.ask", json!({"q": 1})).await.unwrap();
        assert_eq!(
            answer,
            json!({"seq": 7, "theme": "dark", "status": {"version": "0.0.0"}})
        );
        // Host errors reach the module with their kind, and its errors
        // reach the host.
        let denied = host.call_raw("rec.fail", json!({})).await.unwrap_err();
        assert!(matches!(&denied, CallError::Rpc(e) if e.data.kind == ErrorKind::PermissionDenied));
        let unknown = host.call_raw("rec.nope", json!({})).await.unwrap_err();
        assert!(matches!(&unknown, CallError::Rpc(e) if e.data.kind == ErrorKind::NotFound));

        // Events arrive as notifications.
        let event: StoredEvent = serde_json::from_value(json!({
            "seq": 3, "prev_hash": "", "hash": "",
            "event": {"id": "01J00000000000000000000000", "ts": "2026-10-20T00:00:00Z",
                      "type": "setting.changed", "version": 1, "stream": "settings",
                      "correlation": null, "causation": null, "actor": "user", "payload": {}}
        }))
        .unwrap();
        host.notify(EVENT_NOTIFICATION, json!({"event": event}))
            .await
            .unwrap();
        host.call::<Ping>(&Empty {}).await.unwrap();

        host.call::<Shutdown>(&Empty {}).await.unwrap();
        assert_eq!(served.await.unwrap(), Ended::Closed);
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen, ["initialize 9.9.9", "event 3", "shutdown"]);
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn a_module_ends_when_priyad_goes_and_host_calls_then_fail() {
        let (host, served, host_task) = start(Recorder::default());
        host.call::<Initialize>(&init()).await.unwrap();
        // priyad's side stops: dropping its peer and task closes the
        // channel the module reads.
        host_task.abort();
        drop(host);
        assert_eq!(served.await.unwrap(), Ended::Closed);

        assert_eq!(host_error(CallError::Closed).kind, ErrorKind::Unavailable);
        assert_eq!(
            host_error(CallError::TooLarge(1)).kind,
            ErrorKind::InvalidInput
        );
        assert_eq!(
            host_error(CallError::Mismatch("x".into())).kind,
            ErrorKind::Internal
        );
        let bad: Result<InitializeParams, _> = params(json!({"api": 1}));
        assert_eq!(bad.unwrap_err().kind, ErrorKind::InvalidInput);
        assert!(params::<Empty>(Value::Null).is_ok());
    }
}
