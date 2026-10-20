//! The echo module from priyad's side, without priyad: a fake host on the
//! other end of its stdio, and of its builtin channels.

use std::process::Stdio;
use std::time::Duration;

use priya_api::error::{ApiError, ErrorKind};
use priya_api::framing::lsp::{read_messages, write_messages};
use priya_api::module::protocol::{Empty, Initialize, InitializeParams, Ping, Shutdown};
use priya_api::peer::{CallError, Ended, Inbound, Peer};
use priya_api::rpc::RpcError;
use serde_json::{Value, json};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// What the fake host answers: a prefix setting, a denied key, event seqs.
async fn host(method: String, params: Option<Value>) -> Result<Value, RpcError> {
    let params = params.unwrap_or_default();
    match (method.as_str(), params["key"].as_str()) {
        ("host.settings.get", Some("modules.echo.prefix")) => Ok(json!({"value": "> "})),
        ("host.settings.get", _) => Err(RpcError::from_api(ApiError::new(
            ErrorKind::PermissionDenied,
            "The echo module may not read that setting.",
        ))),
        ("host.emit_event", _) if params["custom_type"] == "said" => Ok(json!({"seq": 42})),
        _ => Err(RpcError::method_not_found(&method)),
    }
}

fn init() -> InitializeParams {
    InitializeParams {
        api: "v1".into(),
        host_version: "0.0.0".into(),
    }
}

/// The checks every form of echo must pass.
async fn converse(echo: &Peer) {
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

    let kind = |outcome: Result<Value, CallError>| match outcome {
        Err(CallError::Rpc(error)) => error.data.kind,
        other => panic!("an error, not {other:?}"),
    };
    let denied = echo
        .call_raw("echo.setting", json!({"key": "ui.theme"}))
        .await;
    assert_eq!(kind(denied), ErrorKind::PermissionDenied);
    let wrong = echo.call_raw("echo.say", json!({"words": "hi"})).await;
    assert_eq!(kind(wrong), ErrorKind::InvalidInput);
    let unknown = echo.call_raw("echo.shout", json!({})).await;
    assert_eq!(kind(unknown), ErrorKind::NotFound);

    echo.call::<Shutdown>(&Empty {})
        .await
        .expect("says goodbye");
}

/// The echo binary, with a fake host on its stdio.
fn spawn() -> (Child, Peer, tokio::task::JoinHandle<Ended>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_echo-module"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
    let (peer, task) = Peer::spawn(inbound, outbound, |_| host);
    (child, peer, task)
}

async fn exit_code(child: &mut Child) -> Option<i32> {
    tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("exits in time")
        .expect("waits")
        .code()
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
    echo.call::<Initialize>(&init()).await.unwrap();
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
    let (to_echo, echo_in) = mpsc::channel::<Inbound>(8);
    let (echo_out, mut from_echo) = mpsc::channel::<Vec<u8>>(8);
    let (to_host, host_in) = mpsc::channel::<Inbound>(8);
    let (host_out, mut from_host) = mpsc::channel::<Vec<u8>>(8);
    tokio::spawn(async move {
        while let Some(body) = from_echo.recv().await {
            let _ = to_host.send(Ok(body)).await;
        }
    });
    tokio::spawn(async move {
        while let Some(body) = from_host.recv().await {
            let _ = to_echo.send(Ok(body)).await;
        }
    });
    let served = tokio::spawn((priya_echo_example::BUILTIN)(echo_in, echo_out));
    let (echo, _task) = Peer::spawn(host_in, host_out, |_| host);
    converse(&echo).await;
    assert_eq!(served.await.unwrap(), Ended::Closed);
}
