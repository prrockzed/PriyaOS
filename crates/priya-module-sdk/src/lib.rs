//! Write PriyaOS modules in Rust (02-core §5).
//!
//! A module is a type that implements [`Module`]: it says who it is when
//! priyad starts it, then answers its methods. [`HostClient`] is how it
//! talks back to priyad: record events, read settings, call the read-only
//! public API. The SDK answers `ping` and `shutdown` itself.
//!
//! The same module runs two ways, with the same messages:
//! - as its own process, with [`run_stdio`] in `main` (`kind = "process"`);
//! - inside priyad, through [`builtin`] (`kind = "builtin"`).
//!
//! ```no_run
//! use priya_module_sdk::{ApiError, HostClient, InitializeParams, InitializeResult, Module, Value, json};
//!
//! #[derive(Default)]
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
//!
//! [`testing::connect`] runs a module against a stand-in for priyad, for
//! the module's own tests.

use std::future::Future;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use priya_api::framing::lsp::{read_messages_blocking, write_messages};
use priya_api::methods::{Method, decode_params};
use priya_api::module::protocol::{
    CallApi, CallApiParams, DeliverEvent, EmitEvent, EmitEventParams, EventDelivery, GetSetting,
    GetSettingParams, Initialize, Ping, Shutdown,
};
use priya_api::rpc::RpcError;
use serde::de::DeserializeOwned;

pub use priya_api::error::{ApiError, ErrorKind};
pub use priya_api::methods::events::StoredEvent;
pub use priya_api::module::protocol::{InitializeParams, InitializeResult};
pub use priya_api::peer::{BoxFuture, CallError, Ended, Handler, Inbound, Peer};
pub use serde_json::{Value, json};
pub use tokio::sync::mpsc;

/// Messages waiting in each direction.
const QUEUE: usize = 64;

/// A PriyaOS module.
///
/// # Concurrency
///
/// priyad may call several methods at once (up to 64), and each runs as a
/// task of its own on a multi-threaded runtime, so `handle` takes `&self`
/// and the module must be `Sync`. Keep shared state in atomics or a
/// `Mutex`, and use `tokio::sync::Mutex` for a lock held across an
/// `.await`. Events arrive one at a time, in order, but alongside method
/// calls.
pub trait Module: Send + Sync + 'static {
    /// Called once, first. Returns who the module is; priyad refuses it
    /// unless this matches its `module.toml`. Until it succeeds the
    /// module's methods and events are refused.
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

    /// An event the module subscribes to (`[subscribes] events`). priyad
    /// sends the next one only after this returns.
    fn event(&self, host: &HostClient, event: StoredEvent) -> impl Future<Output = ()> + Send {
        let _ = (host, event);
        async {}
    }

    /// Called last, once priyad has asked the module to stop (or gone
    /// away) and every request in flight is answered. priyad can no longer
    /// be reached by then; keep it short.
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
pub fn parse_params<P: DeserializeOwned>(params: Value) -> Result<P, ApiError> {
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
/// or goes away, then runs its `shutdown` hook; returns how the
/// conversation ended.
pub async fn serve<M: Module>(
    module: M,
    inbound: mpsc::Receiver<Inbound>,
    outbound: mpsc::Sender<Vec<u8>>,
) -> Ended {
    let module = Arc::new(module);
    let (_peer, task) = Peer::spawn(inbound, outbound, {
        let module = module.clone();
        move |peer| Dispatch {
            module,
            host: HostClient { peer },
            initialized: Arc::new(AtomicBool::new(false)),
        }
    });
    let ended = task
        .await
        .unwrap_or_else(|e| Ended::Broken(format!("the module stopped: {e}")));
    module.shutdown().await;
    ended
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
/// goes away, and returns once every answer is written out. stdin is read
/// on a thread of its own, so this works under any runtime.
pub async fn serve_stdio<M: Module>(module: M) -> Ended {
    let (to_module, inbound) = mpsc::channel(QUEUE);
    let (outbound, to_host) = mpsc::channel(QUEUE);
    // Detached: a blocking read can't be interrupted, and the thread ends
    // with the process.
    std::thread::spawn(move || read_messages_blocking(std::io::stdin().lock(), to_module));
    let writer = tokio::spawn(write_messages(tokio::io::stdout(), to_host));
    let ended = serve(module, inbound, outbound).await;
    finished(ended, writer.await)
}

/// How a stdio conversation ended, given how the writer did. A write that
/// failed means priyad stopped reading, which ends things the same way as
/// it going away; only a writer that crashed is a fault.
fn finished(ended: Ended, writer: Result<std::io::Result<()>, tokio::task::JoinError>) -> Ended {
    match writer {
        Err(e) => Ended::Broken(format!("writing stopped: {e}")),
        Ok(_) => ended,
    }
}

/// `main` for a process module: serves `module` on stdio and exits 0 when
/// told to stop or when priyad goes away, or 1 (saying why on stderr) if
/// the conversation broke. It starts its own runtime, so call it from a
/// plain `fn main`, not from inside one.
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
    exit_code(runtime.block_on(serve_stdio(module)))
}

fn exit_code(ended: Ended) -> ExitCode {
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
    initialized: Arc<AtomicBool>,
}

impl<M: Module> Handler for Dispatch<M> {
    fn handle(&self, method: String, params: Option<Value>) -> BoxFuture<Result<Value, RpcError>> {
        let (module, host, initialized) = (
            self.module.clone(),
            self.host.clone(),
            self.initialized.clone(),
        );
        Box::pin(async move { dispatch(&*module, &host, &initialized, method, params).await })
    }

    /// priyad sends modules no notifications: one is ignored, and noted.
    fn notified(&self, method: String, _: Option<Value>) -> BoxFuture<()> {
        report(&format!("ignored a notification: {method}"));
        Box::pin(async {})
    }
}

async fn dispatch<M: Module>(
    module: &M,
    host: &HostClient,
    initialized: &AtomicBool,
    method: String,
    params: Option<Value>,
) -> Result<Value, RpcError> {
    let params = params.unwrap_or(Value::Null);
    match method.as_str() {
        Initialize::NAME if initialized.load(Ordering::Acquire) => Err(RpcError::from_api(
            ApiError::new(ErrorKind::Conflict, "The module has already started."),
        )),
        Initialize::NAME => {
            let params = decode_params(Some(params))?;
            let hello = module
                .initialize(host, params)
                .await
                .map_err(RpcError::from_api)?;
            initialized.store(true, Ordering::Release);
            to_value(&hello)
        }
        Ping::NAME => Ok(json!({})),
        Shutdown::NAME => {
            // Stop taking requests; this answer and those in flight still
            // go out, then `serve` runs the hook.
            host.peer.close();
            Ok(json!({}))
        }
        _ if !initialized.load(Ordering::Acquire) => Err(RpcError::from_api(ApiError::new(
            ErrorKind::Unavailable,
            "The module is still starting.",
        ))),
        DeliverEvent::NAME => {
            let delivery: EventDelivery = decode_params(Some(params))?;
            module.event(host, delivery.event).await;
            Ok(json!({}))
        }
        _ => module
            .handle(host, &method, params)
            .await
            .map_err(RpcError::from_api),
    }
}

fn to_value(value: &impl serde::Serialize) -> Result<Value, RpcError> {
    serde_json::to_value(value).map_err(|e| RpcError::internal(e.to_string()))
}

/// Helpers for a module's own tests.
pub mod testing {
    use priya_api::peer::{Ended, Handler, Inbound, Peer};
    use tokio::sync::mpsc;
    use tokio::task::JoinHandle;

    use crate::{Module, serve};

    /// Serves `module` on channels against a stand-in for priyad, which
    /// answers its host calls with the handler `host` returns. Gives back
    /// priyad's end of the conversation and the module's task.
    pub fn connect<M: Module, H: Handler>(
        module: M,
        host: impl FnOnce(Peer) -> H,
    ) -> (Peer, JoinHandle<Ended>) {
        let (to_module, module_in) = mpsc::channel::<Inbound>(super::QUEUE);
        let (module_out, from_module) = mpsc::channel::<Vec<u8>>(super::QUEUE);
        let (to_host, host_in) = mpsc::channel::<Inbound>(super::QUEUE);
        let (host_out, from_host) = mpsc::channel::<Vec<u8>>(super::QUEUE);
        forward(from_module, to_host);
        forward(from_host, to_module);
        let served = tokio::spawn(serve(module, module_in, module_out));
        let (priyad, _) = Peer::spawn(host_in, host_out, host);
        (priyad, served)
    }

    /// Carries one side's messages to the other.
    fn forward(mut from: mpsc::Receiver<Vec<u8>>, to: mpsc::Sender<Inbound>) {
        tokio::spawn(async move {
            while let Some(body) = from.recv().await {
                if to.send(Ok(body)).await.is_err() {
                    break;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use priya_api::module::protocol::Empty;
    use tokio::sync::oneshot;

    use super::*;

    /// A module that reports what it saw.
    #[derive(Default)]
    struct Recorder {
        seen: Arc<Mutex<Vec<String>>>,
        /// Fired when the third event is in.
        third: Mutex<Option<oneshot::Sender<()>>>,
        refuse: bool,
    }

    impl Recorder {
        fn saw(&self, what: &str) {
            self.seen.lock().unwrap().push(what.to_owned());
        }
    }

    impl Module for Recorder {
        async fn initialize(
            &self,
            _: &HostClient,
            params: InitializeParams,
        ) -> Result<InitializeResult, ApiError> {
            if self.refuse {
                return Err(ApiError::new(ErrorKind::Unavailable, "Not today."));
            }
            self.saw(&format!("initialize {}", params.host_version));
            Ok(InitializeResult::v1(
                "rec",
                "1.0.0",
                ["rec.ask", "rec.fail", "rec.wait"],
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
                "rec.wait" => {
                    // A host call that is in flight when priyad goes away.
                    self.saw("waiting");
                    let error = host.setting("slow").await.unwrap_err();
                    self.saw(&format!("wait {}", error.kind));
                    Ok(json!({}))
                }
                _ => Err(no_such_method(method)),
            }
        }

        async fn event(&self, _: &HostClient, event: StoredEvent) {
            self.saw(&format!("event {}", event.seq));
            if event.seq == 3
                && let Some(third) = self.third.lock().unwrap().take()
            {
                let _ = third.send(());
            }
        }

        async fn shutdown(&self) {
            self.saw("shutdown");
        }
    }

    /// A stand-in for priyad answering the host calls.
    async fn fake_host(method: String, params: Option<Value>) -> Result<Value, RpcError> {
        let key = params
            .as_ref()
            .and_then(|p| p["key"].as_str())
            .unwrap_or_default();
        match (method.as_str(), key) {
            ("host.emit_event", _) => Ok(json!({"seq": 7})),
            ("host.settings.get", "secret") => Err(RpcError::from_api(ApiError::new(
                ErrorKind::PermissionDenied,
                "Not yours.",
            ))),
            ("host.settings.get", "slow") => {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(json!({"value": null}))
            }
            ("host.settings.get", _) => Ok(json!({"value": "dark"})),
            ("host.call", _) => Ok(json!({"version": "0.0.0"})),
            _ => Err(RpcError::method_not_found(&method)),
        }
    }

    fn host(_: Peer) -> fn(String, Option<Value>) -> BoxFuture<Result<Value, RpcError>> {
        |method, params| Box::pin(fake_host(method, params))
    }

    fn init() -> InitializeParams {
        InitializeParams {
            api: "v1".into(),
            host_version: "9.9.9".into(),
        }
    }

    fn event(seq: i64) -> Value {
        json!({"event": {
            "seq": seq, "prev_hash": "", "hash": "",
            "event": {"id": "01J00000000000000000000000", "ts": "2026-10-20T00:00:00Z",
                      "type": "setting.changed", "version": 1, "stream": "settings",
                      "correlation": null, "causation": null, "actor": "user", "payload": {}}
        }})
    }

    fn kind(outcome: Result<Value, CallError>) -> ErrorKind {
        match outcome {
            Err(CallError::Rpc(error)) => error.data.kind,
            other => panic!("an error, not {other:?}"),
        }
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_module_answers_after_initialize_and_stops_on_shutdown() {
        let (third, third_in) = oneshot::channel();
        let module = Recorder {
            third: Mutex::new(Some(third)),
            ..Recorder::default()
        };
        let seen = module.seen.clone();
        let (priyad, served) = testing::connect(module, host);

        // Nothing but the protocol's own methods before initialize.
        assert_eq!(
            kind(priyad.call_raw("rec.ask", json!({})).await),
            ErrorKind::Unavailable
        );
        assert_eq!(
            kind(priyad.call_raw("event", event(1)).await),
            ErrorKind::Unavailable
        );
        priyad.call::<Ping>(&Empty {}).await.unwrap();

        let hello = priyad.call::<Initialize>(&init()).await.unwrap();
        assert_eq!(hello.api, "v1");
        assert_eq!((hello.name.as_str(), hello.methods.len()), ("rec", 3));
        let again = priyad.call::<Initialize>(&init()).await;
        assert!(matches!(again, Err(CallError::Rpc(e)) if e.data.kind == ErrorKind::Conflict));

        // The module's method calls the host three ways.
        let answer = priyad.call_raw("rec.ask", json!({"q": 1})).await.unwrap();
        assert_eq!(
            answer,
            json!({"seq": 7, "theme": "dark", "status": {"version": "0.0.0"}})
        );
        // Host errors reach the module with their kind, and its errors
        // reach the host.
        assert_eq!(
            kind(priyad.call_raw("rec.fail", json!({})).await),
            ErrorKind::PermissionDenied
        );
        assert_eq!(
            kind(priyad.call_raw("rec.nope", json!({})).await),
            ErrorKind::NotFound
        );
        assert_eq!(
            kind(priyad.call_raw("event", json!({"event": 1})).await),
            ErrorKind::InvalidInput
        );

        // Events arrive one call at a time; notifications are ignored.
        for seq in 2..=3 {
            priyad.call_raw("event", event(seq)).await.unwrap();
        }
        priyad.notify("rec.ask", json!({})).await.unwrap();
        third_in.await.unwrap();

        priyad.call::<Shutdown>(&Empty {}).await.unwrap();
        assert_eq!(served.await.unwrap(), Ended::Closed);
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen, ["initialize 9.9.9", "event 2", "event 3", "shutdown"]);
    }

    // covers: CORE-02
    // suite: unit
    // Paused time: priyad's side waits out its drain time for the slow
    // host call before the module sees it gone.
    #[tokio::test(start_paused = true)]
    async fn a_module_ends_when_priyad_goes_and_its_host_calls_fail() {
        let module = Recorder::default();
        let seen = module.seen.clone();
        let (priyad, served) = testing::connect(module, host);
        priyad.call::<Initialize>(&init()).await.unwrap();
        // A method waiting on a host call when priyad goes away: the call
        // fails as unavailable, the module answers and ends.
        let waiting = tokio::spawn({
            let priyad = priyad.clone();
            async move { priyad.call_raw("rec.wait", json!({})).await }
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !seen.lock().unwrap().iter().any(|s| s == "waiting") {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the method starts");
        priyad.close();
        drop(priyad);
        assert_eq!(served.await.unwrap(), Ended::Closed);
        let seen = seen.lock().unwrap().clone();
        assert_eq!(
            seen,
            [
                "initialize 9.9.9",
                "waiting",
                "wait unavailable",
                "shutdown"
            ]
        );
        waiting.abort();
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn a_failed_initialize_keeps_the_module_refusing() {
        let module = Recorder {
            refuse: true,
            ..Recorder::default()
        };
        let (priyad, _served) = testing::connect(module, host);
        let refused = priyad.call::<Initialize>(&init()).await;
        assert!(matches!(refused, Err(CallError::Rpc(e)) if e.data.user_message == "Not today."));
        assert_eq!(
            kind(priyad.call_raw("rec.ask", json!({})).await),
            ErrorKind::Unavailable
        );
        let positional = priyad.call_raw("initialize", json!([1])).await;
        assert_eq!(kind(positional), ErrorKind::InvalidInput);

        assert_eq!(host_error(CallError::Closed).kind, ErrorKind::Unavailable);
        assert_eq!(
            host_error(CallError::TooLarge(1)).kind,
            ErrorKind::InvalidInput
        );
        assert_eq!(
            host_error(CallError::Mismatch("x".into())).kind,
            ErrorKind::Internal
        );
        let bad: Result<InitializeParams, _> = parse_params(json!({"api": 1}));
        assert_eq!(bad.unwrap_err().kind, ErrorKind::InvalidInput);
        assert!(parse_params::<Empty>(Value::Null).is_ok());
        assert_eq!(exit_code(Ended::Closed), ExitCode::SUCCESS);
        assert_eq!(exit_code(Ended::Broken("x".into())), ExitCode::FAILURE);
        let pipe = Ok(Err(std::io::Error::other("pipe")));
        assert_eq!(finished(Ended::Closed, pipe), Ended::Closed);
    }
}
