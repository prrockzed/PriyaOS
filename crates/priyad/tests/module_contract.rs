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
