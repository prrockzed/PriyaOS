//! Modules that misbehave, through the real priyad (0.8): whatever a
//! module does, priyad and the other modules carry on.
//!
//! The fake module (`priya-fake-module`), the echo module and the CLI are
//! found next to priyad, so build them first (`cargo test --workspace`
//! does).

use std::fs;
use std::process::Output;
use std::time::Duration;

use priya_testkit::{FakeModuleProcess, TestDaemon, workspace_binary};
use serde_json::{Value, json};

const PRIYAD: &str = env!("CARGO_BIN_EXE_priyad");
/// Fast pings and restarts, so failures show quickly.
const TIMINGS: &str =
    "ping_every=200,ping_timeout=300,stop_grace=500,restart_first=50,restart_max=200";

fn daemon() -> TestDaemon {
    TestDaemon::start_with(PRIYAD, &[("PRIYA_MODULE_TIMINGS", TIMINGS)]).expect("priyad starts")
}

fn priya(daemon: &TestDaemon, args: &[&str]) -> Output {
    daemon
        .command(workspace_binary("priya"))
        .args(args)
        .output()
        .expect("priya runs")
}

fn ok(daemon: &TestDaemon, args: &[&str]) -> String {
    let output = priya(daemon, args);
    assert!(
        output.status.success(),
        "priya {args:?} failed: {}\npriyad's log:\n{}",
        String::from_utf8_lossy(&output.stderr),
        daemon.log()
    );
    String::from_utf8(output.stdout).expect("UTF-8")
}

/// A call's answer: the result, or the refusal's message.
fn call(daemon: &TestDaemon, method: &str, params: &Value) -> Result<Value, String> {
    let output = priya(
        daemon,
        &["--json", "module", "call", method, &params.to_string()],
    );
    if output.status.success() {
        Ok(serde_json::from_slice(&output.stdout).expect("JSON"))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

fn until(daemon: &TestDaemon, name: &str, ready: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..500 {
        let list: Value =
            serde_json::from_str(&ok(daemon, &["--json", "module", "list"])).expect("JSON");
        let module = list["modules"]
            .as_array()
            .and_then(|all| all.iter().find(|m| m["name"] == name))
            .cloned();
        if let Some(module) = module
            && ready(&module)
        {
            return module;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("{name} didn't get there; priyad's log:\n{}", daemon.log());
}

fn running(module: &Value) -> bool {
    module["state"] == "running"
}

/// The module events so far, as `(type, payload)`.
fn module_events(daemon: &TestDaemon) -> Vec<(String, Value)> {
    let mut tail = daemon
        .command(workspace_binary("priya"))
        .args([
            "--json", "events", "tail", "--from", "0", "--type", "module.*",
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("priya runs");
    std::thread::sleep(Duration::from_millis(500));
    let _ = tail.kill();
    let output = tail.wait_with_output().expect("waits");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| {
            let event: Value = serde_json::from_str(line).expect("one event per line");
            (
                event["event"]["type"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                event["event"]["payload"].clone(),
            )
        })
        .collect()
}

/// Installs the echo module alongside, to see others aren't touched.
fn install_echo(daemon: &TestDaemon) {
    let dir = daemon.modules_dir().join("echo");
    fs::create_dir_all(&dir).expect("a folder");
    let source = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../modules/examples/echo/module.toml"
    );
    let manifest = fs::read_to_string(source).expect("the manifest").replace(
        "exec = \"echo-module\"",
        &format!("exec = \"{}\"", workspace_binary("echo-module").display()),
    );
    let manifest = manifest.replace("settings_schema = \"settings.schema.json\"\n", "");
    fs::write(dir.join("module.toml"), manifest).expect("written");
}

/// Priyad and echo still answer.
fn others_unharmed(daemon: &TestDaemon) {
    ok(daemon, &["status"]);
    let said = call(daemon, "echo.say", &json!({"text": "still here"}));
    assert_eq!(said, Ok(json!({"text": "still here"})));
}

// covers: CORE-02
// suite: integration
// acceptance: 0.8
#[test]
fn crashed_module_restarts_and_emits_events() {
    let d = daemon();
    FakeModuleProcess::new("flaky")
        .crash_on_call(1)
        .install(&d.modules_dir());
    ok(&d, &["module", "enable", "flaky"]);
    until(&d, "flaky", running);
    let answer = call(&d, "flaky.call", &json!({}));
    assert_eq!(
        answer,
        Err("The flaky module stopped before it answered.".into())
    );
    let again = until(&d, "flaky", |m| running(m) && m["restarts"] == 1);
    assert!(
        again["problem"]
            .as_str()
            .unwrap_or_default()
            .contains("exited with code 3"),
        "{again}"
    );

    let events = module_events(&d);
    let types: Vec<&str> = events.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(
        types,
        ["module.started", "module.crashed", "module.started"]
    );
    let crashed = &events[1].1;
    assert_eq!(crashed["exit_code"], 3);
    assert_eq!(crashed["gave_up"], false);
    assert_eq!(crashed["stderr_tail"], "crashing on purpose on call 1");
    // The new run counts its calls afresh.
    assert_eq!(
        call(&d, "flaky.call", &json!({})),
        Err("The flaky module stopped before it answered.".into())
    );
}

// covers: CORE-02
// suite: integration
// acceptance: 0.8
#[test]
fn hung_module_killed_after_ping_timeout() {
    let d = daemon();
    install_echo(&d);
    FakeModuleProcess::new("stuck")
        .hang()
        .install(&d.modules_dir());
    ok(&d, &["module", "enable", "echo"]);
    ok(&d, &["module", "enable", "stuck"]);
    let stuck = until(&d, "stuck", |m| m["restarts"].as_u64() >= Some(1));
    assert!(
        stuck["problem"]
            .as_str()
            .unwrap_or_default()
            .contains("didn't answer a ping within 300 ms"),
        "{stuck}"
    );
    let crashed = module_events(&d)
        .into_iter()
        .find(|(t, p)| t == "module.crashed" && p["module"] == "stuck")
        .expect("recorded");
    assert_eq!(crashed.1["signal"], 9, "killed");
    others_unharmed(&d);
}

// covers: CORE-02
// suite: integration
// acceptance: 0.8
#[test]
fn garbage_output_fails_module_not_daemon() {
    let d = daemon();
    install_echo(&d);
    FakeModuleProcess::new("noisy")
        .garbage()
        .install(&d.modules_dir());
    FakeModuleProcess::new("huge")
        .oversize()
        .install(&d.modules_dir());
    ok(&d, &["module", "enable", "echo"]);
    ok(&d, &["module", "enable", "noisy"]);
    ok(&d, &["module", "enable", "huge"]);
    let noisy = until(&d, "noisy", |m| m["restarts"].as_u64() >= Some(1));
    let problem = noisy["problem"].as_str().unwrap_or_default();
    assert!(
        problem.contains("broke the protocol") && problem.contains("bad message header"),
        "{problem}"
    );
    let huge = until(&d, "huge", |m| m["restarts"].as_u64() >= Some(1));
    assert!(
        huge["problem"]
            .as_str()
            .unwrap_or_default()
            .contains("over the"),
        "{huge}"
    );
    others_unharmed(&d);
}

// covers: CORE-02
// suite: integration
// acceptance: 0.8
#[test]
fn handshake_mismatch_refused() {
    let d = daemon();
    FakeModuleProcess::new("liar")
        .lie()
        .install(&d.modules_dir());
    ok(&d, &["module", "enable", "liar"]);
    let liar = until(&d, "liar", |m| m["state"] == "failed");
    assert_eq!(liar["restarts"], 0, "not tried again");
    let problem = liar["problem"].as_str().unwrap_or_default();
    assert!(problem.contains("not listed: liar.secret"), "{problem}");
    let events = module_events(&d);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].0, "module.crashed");
    assert_eq!(events[0].1["gave_up"], true);
    // Shown plainly in the list.
    let list = ok(&d, &["module", "list"]);
    assert!(list.contains("liar  failed"), "{list}");
}

// covers: CORE-02
// suite: integration
#[test]
fn a_module_that_keeps_crashing_stops_after_six_with_gave_up() {
    let d = daemon();
    install_echo(&d);
    // It crashes on its first call; each restart is called again at once.
    FakeModuleProcess::new("flaky")
        .crash_on_call(1)
        .install(&d.modules_dir());
    ok(&d, &["module", "enable", "echo"]);
    ok(&d, &["module", "enable", "flaky"]);
    for round in 0..6 {
        until(&d, "flaky", running);
        let answer = call(&d, "flaky.call", &json!({}));
        assert!(answer.is_err(), "round {round}");
        if round < 5 {
            until(&d, "flaky", |m| m["restarts"] == round + 1);
        }
    }
    let flaky = until(&d, "flaky", |m| m["state"] == "failed");
    assert_eq!(flaky["restarts"], 5);
    let crashes: Vec<Value> = module_events(&d)
        .into_iter()
        .filter(|(t, p)| t == "module.crashed" && p["module"] == "flaky")
        .map(|(_, p)| p)
        .collect();
    assert_eq!(crashes.len(), 6);
    assert_eq!(crashes[5]["gave_up"], true);
    others_unharmed(&d);
    // Turning it on again starts it afresh.
    ok(&d, &["module", "enable", "flaky"]);
    until(&d, "flaky", running);
}

// covers: CORE-02
// suite: security
// acceptance: 0.8
#[test]
fn module_cannot_call_undeclared_host_method() {
    let d = daemon();
    FakeModuleProcess::new("sneaky").install(&d.modules_dir());
    ok(&d, &["module", "enable", "sneaky"]);
    until(&d, "sneaky", running);
    let host = |method: &str, params: Value| {
        call(
            &d,
            "sneaky.host",
            &json!({"method": method, "params": params}),
        )
    };
    // No capabilities: no events, no public API, nothing that doesn't exist.
    let emit = host(
        "host.emit_event",
        json!({"custom_type": "x", "payload": {}}),
    );
    assert_eq!(
        emit,
        Err("The sneaky module isn't allowed to record events.".into())
    );
    let status = host(
        "host.call",
        json!({"method": "v1.system.status", "params": {}}),
    );
    assert_eq!(
        status,
        Err("The sneaky module isn't allowed to call `v1.system.status`.".into())
    );
    let change = host(
        "host.call",
        json!({"method": "v1.settings.set", "params": {"key": "ui.theme", "value": "light"}}),
    );
    assert_eq!(
        change,
        Err("The sneaky module can't make the request `v1.settings.set`.".into())
    );
    let secret = host("host.secret.get", json!({"name": "x"}));
    assert!(secret.is_err());
    // Each refusal is logged with the module and method; nothing changed.
    assert!(d.log().contains("refused a host call"), "{}", d.log());
    assert_eq!(ok(&d, &["settings", "get", "ui.theme"]).trim(), "dark");
    assert!(module_events(&d).iter().all(|(t, _)| t != "module.event"));

    // Declared, the same calls work.
    FakeModuleProcess::new("open")
        .capability("events.emit")
        .capability("api.read")
        .install(&d.modules_dir());
    ok(&d, &["module", "enable", "open"]);
    until(&d, "open", running);
    let emit = call(
        &d,
        "open.host",
        &json!({"method": "host.emit_event", "params": {"custom_type": "hello", "payload": {}}}),
    );
    assert!(emit.is_ok_and(|answer| answer["seq"].is_i64()));
    let status = call(
        &d,
        "open.host",
        &json!({"method": "host.call", "params": {"method": "v1.system.status", "params": {}}}),
    );
    assert!(status.is_ok_and(|answer| answer["version"].is_string()));
}

// covers: CORE-02
// suite: security
// acceptance: 0.8
#[test]
fn module_cannot_read_other_modules_settings() {
    let d = daemon();
    FakeModuleProcess::new("nosy")
        .capability("settings.read:ui")
        .capability("api.read")
        .install(&d.modules_dir());
    ok(
        &d,
        &["settings", "set", "modules.echo.prefix", r#""secret> ""#],
    );
    ok(&d, &["settings", "set", "modules.nosy.depth", "3"]);
    ok(&d, &["module", "enable", "nosy"]);
    until(&d, "nosy", running);
    let read = |key: &str| {
        call(
            &d,
            "nosy.host",
            &json!({"method": "host.settings.get", "params": {"key": key}}),
        )
    };
    assert_eq!(
        read("modules.nosy.depth"),
        Ok(json!({"value": 3})),
        "its own"
    );
    assert_eq!(read("ui.theme"), Ok(json!({"value": "dark"})), "declared");
    assert_eq!(
        read("modules.echo.prefix"),
        Err("The nosy module isn't allowed to read the setting `modules.echo.prefix`.".into())
    );
    assert!(read("modules").is_err(), "not the whole section");
    assert!(read("ai.strategy").is_err());
    // Not through the public API either.
    let through_api = |params: Value| {
        call(
            &d,
            "nosy.host",
            &json!({"method": "host.call", "params": {"method": "v1.settings.get", "params": params}}),
        )
    };
    assert!(through_api(json!({})).is_err(), "all settings");
    assert!(through_api(json!({"key": "modules.echo.prefix"})).is_err());
    assert!(through_api(json!({"key": "ui.theme"})).is_ok());
}
