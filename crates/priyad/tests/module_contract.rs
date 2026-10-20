//! The module host contract (0.8): the same scenario against the echo
//! module built into priyad, the Rust echo as a process, and the Python
//! echo as a process. All three must answer alike and leave the same
//! events behind; that is what lets a module move between kinds, and
//! languages, without changing behaviour (02-core §5.3).
//!
//! The process forms need `echo-module` built next to this test
//! (`cargo test --workspace` does) and `python3` on the PATH.

mod common;

use std::fs;
use std::path::Path;
use std::time::Duration;

use priya_api::error::ErrorKind;
use priya_core::settings::KeyPath;
use priya_testkit::workspace_binary;
use priyad::modules::launch::Builtins;
use priyad::modules::{ReportState, Timings};
use serde_json::{Value, json};

/// The echo module's folder in the repository.
const ECHO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../modules/examples/echo");
const ECHO_PY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../modules/examples/echo-py"
);

/// The three forms of echo.
#[derive(Debug, Clone, Copy)]
enum Echo {
    BuiltinRust,
    ProcessRust,
    ProcessPython,
}

/// The builtins `form` needs.
fn builtins(form: Echo) -> Builtins {
    match form {
        Echo::BuiltinRust => Builtins::from([("echo", priya_echo_example::BUILTIN)]),
        Echo::ProcessRust | Echo::ProcessPython => Builtins::new(),
    }
}

/// Installs `echo` in `modules` as `form`.
fn install(form: Echo, modules: &Path) {
    let dir = modules.join("echo");
    fs::create_dir_all(&dir).expect("a folder");
    let (source, kind) = match form {
        Echo::BuiltinRust => (ECHO, "kind = \"builtin\"".to_owned()),
        Echo::ProcessRust => (
            ECHO,
            format!(
                "kind = \"process\"\nexec = \"{}\"",
                workspace_binary("echo-module").display()
            ),
        ),
        Echo::ProcessPython => (
            ECHO_PY,
            format!("kind = \"process\"\nexec = \"{ECHO_PY}/echo.py\""),
        ),
    };
    let manifest = fs::read_to_string(format!("{source}/module.toml")).expect("the manifest");
    let manifest: String = manifest
        .lines()
        .filter(|line| !line.starts_with("exec =") && !line.starts_with("kind ="))
        .map(|line| {
            if line.starts_with("api =") {
                format!("{line}\n{kind}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    fs::write(dir.join("module.toml"), manifest).expect("written");
    fs::copy(
        format!("{source}/settings.schema.json"),
        dir.join("settings.schema.json"),
    )
    .expect("copied");
}

/// What a scenario showed: each call's outcome, and the module events.
#[derive(Debug, PartialEq)]
struct Seen {
    answers: Vec<Value>,
    events: Vec<Value>,
}

/// An answer as data: the result, or the error's kind and message.
fn outcome(answer: Result<Value, priya_api::error::ApiError>) -> Value {
    match answer {
        Ok(result) => json!({"result": result}),
        Err(error) => json!({"kind": error.kind, "message": error.user_message}),
    }
}

/// The scenario: handshake, call and result, its own settings, error
/// mapping, a denied host call, an event, and shutdown.
async fn scenario(form: Echo) -> Seen {
    let timings = Timings {
        ping_every: Duration::from_millis(200),
        stop_grace: Duration::from_secs(2),
        ..Timings::default()
    };
    let machine = common::machine_with(builtins(form), timings).await;
    install(form, &machine.modules);
    let daemon = &machine.daemon;
    let prefix = KeyPath::from_parts(["modules", "echo", "prefix"]);
    daemon
        .settings
        .set(&prefix, &json!("> "))
        .await
        .expect("set");

    let host = &daemon.modules;
    host.set_enabled(&["echo".into()]).await;
    let mut ready = false;
    for _ in 0..500 {
        let state = host
            .list()
            .await
            .into_iter()
            .find(|m| m.name == "echo")
            .map(|m| m.state);
        if state == Some(ReportState::Running) {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(ready, "{form:?} didn't start: {:?}", host.list().await);

    let mut answers = Vec::new();
    for (method, params) in [
        ("echo.say", json!({"text": "hi"})),
        ("echo.say", json!({})),
        ("echo.say", json!({"text": "hi", "loud": true})),
        ("echo.setting", json!({"key": "modules.echo.prefix"})),
        ("echo.setting", json!({"key": "ui.theme"})),
        ("echo.emit", json!({"text": "hi"})),
        ("echo.shout", json!({})),
    ] {
        answers.push(outcome(host.call(method, params).await));
    }
    host.set_enabled(&[]).await;

    let events: Vec<Value> = daemon
        .store
        .query(|conn| {
            let mut statement = conn.prepare(
                "SELECT type, actor, payload FROM events WHERE type LIKE 'module.%' ORDER BY seq",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
        .expect("the events")
        .into_iter()
        .map(|(event_type, actor, payload)| {
            let mut payload: Value = serde_json::from_str(&payload).expect("JSON");
            // A builtin has no process id; the rest must match exactly.
            if event_type == "module.started" {
                payload["pid"] = json!("<pid>");
            }
            json!({"type": event_type, "actor": actor, "payload": payload})
        })
        .collect();
    Seen { answers, events }
}

/// What every form must show.
fn expected() -> Seen {
    let wrong = json!({"kind": ErrorKind::InvalidInput, "message": "That request has a wrong or missing value."});
    Seen {
        answers: vec![
            json!({"result": {"text": "> hi"}}),
            wrong.clone(),
            wrong,
            json!({"result": {"value": "> "}}),
            json!({"kind": "permission_denied", "message": "The echo module isn't allowed to read the setting `ui.theme`."}),
            // The event's seq: two settings events and module.started come first.
            json!({"result": {"seq": 3}}),
            json!({"kind": "not_found", "message": "No module offers `echo.shout`. `priya module list` shows what modules offer."}),
        ],
        events: vec![
            json!({"type": "module.started", "actor": "system", "payload": {"module": "echo", "version": "0.1.0", "pid": "<pid>"}}),
            json!({"type": "module.event", "actor": "module:echo", "payload": {"module": "echo", "name": "said", "data": {"text": "hi"}}}),
            json!({"type": "module.stopped", "actor": "system", "payload": {"module": "echo", "reason": "disabled"}}),
        ],
    }
}

// covers: CORE-02
// suite: contract
// acceptance: 0.8
#[tokio::test]
async fn module_host_contract_builtin_rust_echo() {
    assert_eq!(scenario(Echo::BuiltinRust).await, expected());
}

// covers: CORE-02
// suite: contract
// acceptance: 0.8
#[tokio::test]
async fn module_host_contract_process_rust_echo() {
    assert_eq!(scenario(Echo::ProcessRust).await, expected());
}

// covers: CORE-02
// suite: contract
// acceptance: 0.8
#[tokio::test]
async fn module_host_contract_process_python_echo() {
    assert_eq!(scenario(Echo::ProcessPython).await, expected());
}

/// `echo.py` on its own, with a host stand-in on its stdio that answers
/// the prefix setting.
fn python_echo() -> (
    tokio::process::Child,
    priya_api::peer::Peer,
    tokio::task::JoinHandle<priya_api::peer::Ended>,
) {
    use priya_api::framing::lsp::{read_messages, write_messages};
    use priya_api::rpc::RpcError;
    let mut child = tokio::process::Command::new(format!("{ECHO_PY}/echo.py"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("echo.py starts");
    let (to_host, inbound) = tokio::sync::mpsc::channel(8);
    let (outbound, to_module) = tokio::sync::mpsc::channel(8);
    tokio::spawn(read_messages(child.stdout.take().expect("piped"), to_host));
    tokio::spawn(write_messages(
        child.stdin.take().expect("piped"),
        to_module,
    ));
    let (peer, task) = priya_api::peer::Peer::spawn(inbound, outbound, |_| {
        |method: String, _: Option<Value>| async move {
            match method.as_str() {
                "host.settings.get" => Ok(json!({"value": null})),
                _ => Err(RpcError::method_not_found(&method)),
            }
        }
    });
    (child, peer, task)
}

async fn exit_code(child: &mut tokio::process::Child) -> Option<i32> {
    tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("exits in time")
        .expect("waits")
        .code()
}

fn rpc_kind(outcome: Result<Value, priya_api::peer::CallError>) -> ErrorKind {
    match outcome {
        Err(priya_api::peer::CallError::Rpc(error)) => error.data.kind,
        other => panic!("an error, not {other:?}"),
    }
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn the_python_echo_follows_the_protocol_like_the_sdk() {
    let (mut child, echo, _task) = python_echo();
    // Before initialize, only the protocol's own methods.
    assert_eq!(
        rpc_kind(echo.call_raw("echo.say", json!({"text": "hi"})).await),
        ErrorKind::Unavailable
    );
    assert_eq!(
        rpc_kind(echo.call_raw("initialize", json!({})).await),
        ErrorKind::InvalidInput
    );
    let hello = echo
        .call_raw(
            "initialize",
            json!({"api": "v1", "host_version": "0.0.0", "later": 1}),
        )
        .await
        .expect("initializes");
    // It says what its manifest says.
    let manifest = priya_api::module::ModuleManifest::parse(
        &std::fs::read_to_string(format!("{ECHO_PY}/module.toml")).expect("the manifest"),
    )
    .expect("a valid manifest");
    assert_eq!(hello["name"], manifest.module.name.as_str());
    assert_eq!(hello["version"], manifest.module.version.as_str());
    assert_eq!(hello["methods"], json!(manifest.provides.methods));
    assert_eq!(hello["api"], "v1");
    let again = echo
        .call_raw("initialize", json!({"api": "v1", "host_version": "0"}))
        .await;
    assert_eq!(rpc_kind(again), ErrorKind::Conflict);

    // Notifications are ignored; calls go on.
    echo.notify("echo.emit", json!({"text": "x"}))
        .await
        .expect("sent");
    assert_eq!(
        echo.call_raw("echo.say", json!({"text": "hi"}))
            .await
            .expect("says"),
        json!({"text": "hi"})
    );
    assert_eq!(
        rpc_kind(echo.call_raw("echo.say", json!({"text": 5})).await),
        ErrorKind::InvalidInput
    );
    assert_eq!(
        rpc_kind(echo.call_raw("echo.shout", json!({})).await),
        ErrorKind::NotFound
    );
    assert_eq!(
        rpc_kind(echo.call_raw("event", json!({"event": 1})).await),
        ErrorKind::InvalidInput
    );
    assert_eq!(
        echo.call_raw("ping", json!({})).await.expect("pongs"),
        json!({})
    );

    echo.call_raw("shutdown", json!({}))
        .await
        .expect("says goodbye");
    assert_eq!(exit_code(&mut child).await, Some(0));

    // It ends when priyad goes away, and says why when it is sent garbage.
    let (mut child, echo, task) = python_echo();
    // Its stdin closes once priyad's side has gone.
    task.abort();
    drop(echo);
    assert_eq!(exit_code(&mut child).await, Some(0));
    for (garbage, why) in [
        (&b"not a header\r\n\r\n"[..], "bad message header"),
        (b"Content-Length: 3\r\n\r\nNaN", "isn't JSON"),
    ] {
        let mut child = tokio::process::Command::new(format!("{ECHO_PY}/echo.py"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("echo.py starts");
        let mut stdin = child.stdin.take().expect("piped");
        tokio::io::AsyncWriteExt::write_all(&mut stdin, garbage)
            .await
            .expect("written");
        assert_eq!(exit_code(&mut child).await, Some(1));
        let mut stderr = String::new();
        tokio::io::AsyncReadExt::read_to_string(
            &mut child.stderr.take().expect("piped"),
            &mut stderr,
        )
        .await
        .expect("read");
        assert!(stderr.contains(why), "{stderr}");
    }
}
