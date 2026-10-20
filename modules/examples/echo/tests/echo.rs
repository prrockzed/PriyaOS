//! The echo module from priyad's side, without priyad: a fake host on the
//! other end of its stdio, and of its builtin channels.

use std::process::Stdio;
use std::time::Duration;

use priya_api::error::{ApiError, ErrorKind};
use priya_api::framing::lsp::{read_messages, write_messages};
use priya_api::module::ModuleManifest;
use priya_api::module::protocol::{Empty, Initialize, InitializeParams, Ping, Shutdown};
use priya_api::peer::{BoxFuture, CallError, Ended, Peer};
use priya_api::rpc::RpcError;
use priya_echo_example::{Echo, METHODS, NAME};
use priya_module_sdk::InitializeResult;
use serde_json::{Value, json};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// How long any one conversation may take, even on a slow CI runner.
const PATIENCE: Duration = Duration::from_secs(30);

type Host = fn(String, Option<Value>) -> BoxFuture<Result<Value, RpcError>>;

/// A fake host with the prefix setting `> `, one that has none, and both
/// refusing other keys and checking the `said` event's payload.
fn with_prefix(_: Peer) -> Host {
    |method, params| Box::pin(host(method, params, json!("> ")))
}

fn without_prefix(_: Peer) -> Host {
    |method, params| Box::pin(host(method, params, Value::Null))
}

async fn host(method: String, params: Option<Value>, prefix: Value) -> Result<Value, RpcError> {
    let params = params.unwrap_or_default();
    match (method.as_str(), params["key"].as_str()) {
        ("host.settings.get", Some("modules.echo.prefix")) => Ok(json!({"value": prefix})),
        ("host.settings.get", _) => Err(RpcError::from_api(ApiError::new(
            ErrorKind::PermissionDenied,
            "The echo module may not read that setting.",
        ))),
        ("host.emit_event", _)
            if params == json!({"custom_type": "said", "payload": {"text": "hi"}}) =>
        {
            Ok(json!({"seq": 42}))
        }
        _ => Err(RpcError::method_not_found(&method)),
    }
}

fn init() -> InitializeParams {
    InitializeParams {
        api: "v1".into(),
        host_version: "0.0.0".into(),
    }
}

fn kind(outcome: Result<Value, CallError>) -> ErrorKind {
    match outcome {
        Err(CallError::Rpc(error)) => error.data.kind,
        other => panic!("an error, not {other:?}"),
    }
}

/// The checks every form of echo must pass, against [`with_prefix`].
async fn converse(echo: &Peer) {
    let talk = async {
        let hello = echo.call::<Initialize>(&init()).await.expect("initializes");
        assert_eq!(hello.api, "v1");
        assert_eq!(hello.name, "echo");
        assert_eq!(hello.methods, ["echo.say", "echo.emit", "echo.setting"]);
        echo.call::<Ping>(&Empty {}).await.expect("pongs");

        let said = echo.call_raw("echo.say", json!({"text": "hi"})).await;
        assert_eq!(said.expect("says"), json!({"text": "> hi"}));
        let emitted = echo.call_raw("echo.emit", json!({"text": "hi"})).await;
        assert_eq!(emitted.expect("emits"), json!({"seq": 42}));
        let own = echo
            .call_raw("echo.setting", json!({"key": "modules.echo.prefix"}))
            .await;
        assert_eq!(own.expect("reads its own setting"), json!({"value": "> "}));

        let denied = echo
            .call_raw("echo.setting", json!({"key": "ui.theme"}))
            .await;
        assert_eq!(kind(denied), ErrorKind::PermissionDenied);
        let wrong = echo.call_raw("echo.say", json!({"words": "hi"})).await;
        assert_eq!(kind(wrong), ErrorKind::InvalidInput);
        let extra = echo
            .call_raw("echo.say", json!({"text": "hi", "loud": true}))
            .await;
        assert_eq!(kind(extra), ErrorKind::InvalidInput);
        let unknown = echo.call_raw("echo.shout", json!({})).await;
        assert_eq!(kind(unknown), ErrorKind::NotFound);

        echo.call::<Shutdown>(&Empty {})
            .await
            .expect("says goodbye");
    };
    tokio::time::timeout(PATIENCE, talk)
        .await
        .expect("echo answers in time");
}

/// The echo binary, with a fake host on its stdio.
fn spawn() -> (Child, Peer, tokio::task::JoinHandle<Ended>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_echo-module"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("echo-module starts");
    let (to_host, inbound) = mpsc::channel(8);
    let (outbound, to_module) = mpsc::channel(8);
    tokio::spawn(read_messages(child.stdout.take().expect("piped"), to_host));
    tokio::spawn(write_messages(
        child.stdin.take().expect("piped"),
        to_module,
    ));
    let (peer, task) = Peer::spawn(inbound, outbound, with_prefix);
    (child, peer, task)
}

async fn exit_code(child: &mut Child) -> Option<i32> {
    tokio::time::timeout(PATIENCE, child.wait())
        .await
        .expect("exits in time")
        .expect("waits")
        .code()
}

// covers: CORE-02
// suite: unit
#[test]
fn the_manifest_matches_what_echo_says_it_is() {
    let manifest = ModuleManifest::parse(include_str!("../module.toml")).expect("a valid manifest");
    let said = InitializeResult::v1(NAME, env!("CARGO_PKG_VERSION"), METHODS);
    assert_eq!(manifest.module.name, said.name);
    assert_eq!(manifest.module.version, said.version);
    assert_eq!(manifest.provides.methods, said.methods);

    let schema: Value =
        serde_json::from_str(include_str!("../settings.schema.json")).expect("JSON");
    assert_eq!(schema["properties"]["prefix"]["type"], "string");
    assert_eq!(schema["additionalProperties"], false);
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn echo_process_answers_on_stdio_and_exits_when_told() {
    let (mut child, echo, task) = spawn();
    converse(&echo).await;
    assert_eq!(exit_code(&mut child).await, Some(0));
    assert_eq!(task.await.unwrap(), Ended::Closed);

    // It also exits when priyad goes away (its stdin closes).
    let (mut child, echo, task) = spawn();
    let started = tokio::time::timeout(PATIENCE, echo.call::<Initialize>(&init())).await;
    started.expect("in time").expect("initializes");
    task.abort();
    drop(echo);
    assert_eq!(exit_code(&mut child).await, Some(0));
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn echo_process_exits_1_and_says_why_on_garbage() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_echo-module"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("echo-module starts");
    let mut stdin = child.stdin.take().expect("piped");
    tokio::io::AsyncWriteExt::write_all(&mut stdin, b"not a header\r\n\r\n")
        .await
        .unwrap();
    assert_eq!(exit_code(&mut child).await, Some(1));
    let mut stderr = String::new();
    tokio::io::AsyncReadExt::read_to_string(&mut child.stderr.take().unwrap(), &mut stderr)
        .await
        .unwrap();
    assert!(stderr.contains("bad message header"), "{stderr}");
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn echo_builtin_answers_the_same_over_channels() {
    let (echo, served) = priya_module_sdk::testing::connect(Echo, with_prefix);
    converse(&echo).await;
    assert_eq!(served.await.unwrap(), Ended::Closed);

    // With no prefix set, it says just the text.
    let (echo, _served) = priya_module_sdk::testing::connect(Echo, without_prefix);
    echo.call::<Initialize>(&init()).await.unwrap();
    let said = echo.call_raw("echo.say", json!({"text": "hi"})).await;
    assert_eq!(said.unwrap(), json!({"text": "hi"}));
}
