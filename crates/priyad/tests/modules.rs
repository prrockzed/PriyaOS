//! Modules through the real priyad and the `priya` CLI, each in a
//! temporary home (0.8).
//!
//! The echo module and the CLI are found next to priyad, so build them
//! first (`cargo test --workspace` does).

use std::fs;
use std::process::Output;
use std::time::Duration;

use priya_testkit::{TestDaemon, workspace_binary};
use serde_json::Value;

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

/// The output of a command that must succeed.
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

/// Installs the echo module, as a process, in the user's modules folder.
fn install_echo(daemon: &TestDaemon) {
    let dir = daemon.modules_dir().join("echo");
    fs::create_dir_all(&dir).expect("a folder");
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../modules/examples/echo");
    let manifest = fs::read_to_string(format!("{source}/module.toml")).expect("the manifest");
    let exec = workspace_binary("echo-module");
    let manifest = manifest.replace(
        "exec = \"echo-module\"",
        &format!("exec = \"{}\"", exec.display()),
    );
    fs::write(dir.join("module.toml"), manifest).expect("written");
    fs::copy(
        format!("{source}/settings.schema.json"),
        dir.join("settings.schema.json"),
    )
    .expect("copied");
}

/// The module `name` as `priya module list --json` reports it, once
/// `ready` says yes.
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

fn events(daemon: &TestDaemon) -> Vec<Value> {
    // A tail from the start, cut off once it has caught up.
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
        .map(|line| serde_json::from_str(line).expect("one event per line"))
        .collect()
}

// covers: CORE-02, CORE-04
// suite: integration
#[test]
fn the_cli_turns_the_echo_module_on_calls_it_and_sees_its_events() {
    let d = daemon();
    install_echo(&d);
    assert_eq!(
        ok(&d, &["module", "list"]),
        format!("echo  {:10}  0.1.0\n", "off")
    );
    assert_eq!(ok(&d, &["module", "enable", "echo"]), "Turned on echo.\n");
    let echo = until(&d, "echo", running);
    assert_eq!(echo["enabled"], true);
    assert_eq!(echo["kind"], "process");

    let said = ok(
        &d,
        &["--json", "module", "call", "echo.say", r#"{"text":"hi"}"#],
    );
    assert_eq!(said, "{\"text\":\"hi\"}\n");
    // Its own settings section, live.
    ok(&d, &["settings", "set", "modules.echo.prefix", r#""> ""#]);
    let said = ok(&d, &["module", "call", "echo.say", r#"{"text":"hi"}"#]);
    assert!(said.contains("\"> hi\""), "{said}");
    ok(&d, &["module", "call", "echo.emit", r#"{"text":"hi"}"#]);

    // Refusals come back plainly.
    let denied = priya(
        &d,
        &["module", "call", "echo.setting", r#"{"key":"ui.theme"}"#],
    );
    assert_eq!(denied.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&denied.stderr).trim(),
        "The echo module isn't allowed to read the setting `ui.theme`."
    );
    let bad = priya(&d, &["module", "call", "echo.say", "{not json"]);
    assert_eq!(bad.status.code(), Some(1));
    let missing = priya(&d, &["module", "enable", "ghost"]);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("no module named `ghost`"));

    let status: Value = serde_json::from_str(&ok(&d, &["--json", "status"])).expect("JSON");
    assert_eq!(status["modules"], serde_json::json!(["echo"]));
    let seen: Vec<(String, String)> = events(&d)
        .iter()
        .map(|e| {
            (
                e["event"]["type"].as_str().unwrap_or_default().to_owned(),
                e["event"]["actor"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("module.started".into(), "system".into()),
            ("module.event".into(), "module:echo".into())
        ]
    );
}

// covers: CORE-02
// suite: integration
// acceptance: 0.8
#[test]
fn disable_stops_module_and_persists() {
    let mut d = daemon();
    install_echo(&d);
    ok(&d, &["module", "enable", "echo"]);
    let pid = until(&d, "echo", running)["pid"].as_i64().expect("a pid");
    assert_eq!(ok(&d, &["module", "disable", "echo"]), "Turned off echo.\n");
    let echo = until(&d, "echo", |m| m["state"] == "disabled");
    assert_eq!(echo["enabled"], false);
    // The process is gone, and the change is in the user's settings file.
    let gone = rustix::process::Pid::from_raw(i32::try_from(pid).expect("small"))
        .is_some_and(|pid| rustix::process::test_kill_process(pid).is_err());
    assert!(gone, "the module's process was stopped");
    let file = fs::read_to_string(d.user_settings()).expect("written");
    assert!(file.contains("enabled = []"), "{file}");

    // After a restart it stays off; turned on, it is back after one too.
    d.restart().expect("restarts");
    assert_eq!(until(&d, "echo", |_| true)["state"], "disabled");
    ok(&d, &["module", "enable", "echo"]);
    d.restart().expect("restarts");
    until(&d, "echo", running);
    let stopped = events(&d)
        .into_iter()
        .filter(|e| e["event"]["type"] == "module.stopped")
        .map(|e| {
            e["event"]["payload"]["reason"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(stopped, ["disabled", "priyad is stopping"]);
}

// covers: CORE-02, CORE-03
// suite: integration
// acceptance: 0.8
#[test]
fn module_settings_validated_against_module_schema() {
    let d = daemon();
    install_echo(&d);
    // Not text, as echo's schema requires: refused, and nothing written.
    let refused = priya(&d, &["settings", "set", "modules.echo.prefix", "5"]);
    assert_eq!(refused.status.code(), Some(1));
    let message = String::from_utf8_lossy(&refused.stderr);
    assert!(
        message.starts_with(
            "Nothing was changed: `modules.echo.prefix` doesn't fit the echo module's settings:"
        ),
        "{message}"
    );
    assert!(!d.user_settings().exists(), "nothing written");
    let unknown = priya(&d, &["settings", "set", "modules.echo.colour", r#""red""#]);
    assert_eq!(
        unknown.status.code(),
        Some(1),
        "its schema allows no other keys"
    );
    // Text is fine, and the module sees it.
    ok(&d, &["settings", "set", "modules.echo.prefix", r#""> ""#]);
    ok(&d, &["module", "enable", "echo"]);
    until(&d, "echo", running);
    let said = ok(
        &d,
        &["--json", "module", "call", "echo.say", r#"{"text":"hi"}"#],
    );
    assert_eq!(said, "{\"text\":\"> hi\"}\n");

    // A file edited by hand to break the schema isn't taken: the old value
    // stays, and the problem is recorded.
    fs::write(
        d.user_settings(),
        "[modules]\nenabled = [\"echo\"]\n\n[modules.echo]\nprefix = 5\n",
    )
    .expect("written");
    let mut failed = None;
    for _ in 0..200 {
        let tail = daemon_events(&d, "settings.load_failed");
        if let Some(event) = tail.into_iter().next() {
            failed = Some(event);
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let failed = failed.expect("the broken file is reported");
    assert!(
        failed["event"]["payload"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("doesn't fit the echo module's settings")
    );
    let kept: Value = serde_json::from_str(&ok(
        &d,
        &["--json", "settings", "get", "modules.echo.prefix"],
    ))
    .expect("JSON");
    assert_eq!(kept["value"], "> ", "the old value stays");
    let said = ok(
        &d,
        &["--json", "module", "call", "echo.say", r#"{"text":"hi"}"#],
    );
    assert_eq!(said, "{\"text\":\"> hi\"}\n");
}

/// The events of one type so far.
fn daemon_events(daemon: &TestDaemon, event_type: &str) -> Vec<Value> {
    let mut tail = daemon
        .command(workspace_binary("priya"))
        .args([
            "--json", "events", "tail", "--from", "0", "--type", event_type,
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("priya runs");
    std::thread::sleep(Duration::from_millis(200));
    let _ = tail.kill();
    let output = tail.wait_with_output().expect("waits");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("one event per line"))
        .collect()
}
