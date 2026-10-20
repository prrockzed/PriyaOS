//! The module host on its own (0.8): real modules (builtin and process),
//! with the rest of priyad stood in for by [`Fake`] services.
//!
//! The process tests run `echo-module`, found next to this test, so build
//! it first (`cargo test --workspace` does, or `cargo build -p
//! priya-echo-example`).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use priya_api::error::{ApiError, ErrorKind};
use priya_api::events::NewEvent;
use priya_api::methods::Method;
use priya_api::methods::events::StoredEvent;
use priya_api::module::protocol::{DeliverEvent, InitializeResult};
use priya_api::peer::{BoxFuture, Ended, Inbound, Peer};
use priya_api::rpc::RpcError;
use priya_core::settings::KeyPath;
use priya_testkit::workspace_binary;
use priyad::modules::host_api::HostServices;
use priyad::modules::launch::{Builtins, Spawn};
use priyad::modules::policy::RestartPolicy;
use priyad::modules::{HostConfig, ModuleHost, ModuleReport, ReportState, Timings};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::sync::mpsc;

/// The rest of priyad: records events in a list, has a few settings,
/// answers public calls, and feeds subscribed events from the test.
#[derive(Default)]
struct Fake {
    events: Mutex<Vec<NewEvent>>,
    feed: Mutex<Vec<mpsc::Sender<StoredEvent>>>,
}

impl Fake {
    /// The recorded events as `type actor` lines, with a payload field.
    fn log(&self) -> Vec<(String, String, Value)> {
        self.events
            .lock()
            .expect("not poisoned")
            .iter()
            .map(|e| (e.event_type.clone(), e.actor.to_string(), e.payload.clone()))
            .collect()
    }

    fn types(&self) -> Vec<String> {
        self.log().into_iter().map(|(t, _, _)| t).collect()
    }
}

impl HostServices for Fake {
    fn record(&self, event: NewEvent) -> BoxFuture<Result<i64, ApiError>> {
        let mut events = self.events.lock().expect("not poisoned");
        events.push(event);
        let seq = i64::try_from(events.len()).expect("works in a test");
        Box::pin(async move { Ok(seq) })
    }

    fn setting(&self, key: &KeyPath) -> Value {
        match key.to_string().as_str() {
            "modules.echo.prefix" => json!("> "),
            "ui.theme" => json!("dark"),
            _ => Value::Null,
        }
    }

    fn call_api(&self, method: String, _: Value) -> BoxFuture<Result<Value, ApiError>> {
        Box::pin(async move { Ok(json!({"called": method})) })
    }

    fn subscribe(&self, _: Vec<String>) -> mpsc::Receiver<StoredEvent> {
        let (tx, rx) = mpsc::channel(8);
        self.feed.lock().expect("not poisoned").push(tx);
        rx
    }
}

/// Fast timings, so failures show quickly.
fn timings() -> (Timings, RestartPolicy) {
    (
        Timings {
            handshake: Duration::from_secs(5),
            // Slow enough that a busy CI runner never misses one; the ping
            // test sets its own.
            ping_every: Duration::from_secs(1),
            ping_timeout: Duration::from_millis(300),
            stop_grace: Duration::from_millis(500),
            call_timeout: Duration::from_secs(2),
        },
        RestartPolicy {
            first: Duration::from_millis(20),
            max: Duration::from_millis(100),
            window: Duration::from_secs(30),
            max_failures: 5,
        },
    )
}

struct Setup {
    _dir: TempDir,
    modules: PathBuf,
    fake: Arc<Fake>,
    host: ModuleHost,
}

fn setup(builtins: Builtins) -> Setup {
    let (timings, policy) = timings();
    setup_with(builtins, timings, policy)
}

fn setup_with(builtins: Builtins, timings: Timings, policy: RestartPolicy) -> Setup {
    let dir = tempfile::tempdir().expect("works in a test");
    let modules = dir.path().join("modules");
    fs::create_dir_all(&modules).expect("works in a test");
    let fake = Arc::new(Fake::default());
    let host = ModuleHost::new(
        HostConfig {
            dirs: vec![dir.path().join("system"), modules.clone()],
            builtins,
            timings,
            policy,
            spawn: Spawn::Direct,
        },
        fake.clone(),
    );
    Setup {
        _dir: dir,
        modules,
        fake,
        host,
    }
}

/// Writes `<modules>/<name>/module.toml`.
fn manifest(modules: &Path, name: &str, body: &str) -> PathBuf {
    let dir = modules.join(name);
    fs::create_dir_all(&dir).expect("works in a test");
    fs::write(
        dir.join("module.toml"),
        format!("[module]\nname = \"{name}\"\nversion = \"0.1.0\"\napi = \"v1\"\n{body}"),
    )
    .expect("works in a test");
    dir
}

const ECHO_BODY: &str = "[provides]\nmethods = [\"echo.say\", \"echo.emit\", \"echo.setting\"]\n\
                         [requires]\ncapabilities = [\"events.emit\"]\n";

fn builtin_echo(modules: &Path) {
    manifest(modules, "echo", &format!("kind = \"builtin\"\n{ECHO_BODY}"));
}

fn process_echo(modules: &Path) {
    let exec = workspace_binary("echo-module");
    manifest(
        modules,
        "echo",
        &format!(
            "kind = \"process\"\nexec = \"{}\"\n{ECHO_BODY}",
            exec.display()
        ),
    );
}

fn echo_builtins() -> Builtins {
    Builtins::from([("echo", priya_echo_example::BUILTIN)])
}

/// The module's report once `ready` says yes, within 10 s.
async fn until(
    host: &ModuleHost,
    name: &str,
    ready: impl Fn(&ModuleReport) -> bool,
) -> ModuleReport {
    for _ in 0..1000 {
        if let Some(report) = host.list().await.into_iter().find(|r| r.name == name)
            && ready(&report)
        {
            return report;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{name} didn't get there: {:?}", host.list().await);
}

fn running(report: &ModuleReport) -> bool {
    report.state == ReportState::Running
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn a_builtin_module_answers_records_events_and_stops_when_disabled() {
    let setup = setup(echo_builtins());
    builtin_echo(&setup.modules);
    let host = &setup.host;
    host.set_enabled(&["echo".into()]).await;
    let report = until(host, "echo", running).await;
    assert_eq!(report.pid, None, "a builtin has no process");
    assert_eq!(host.running().await, ["echo"]);

    let said = host.call("echo.say", json!({"text": "hi"})).await.unwrap();
    assert_eq!(said, json!({"text": "> hi"}));
    host.call("echo.emit", json!({"text": "hi"})).await.unwrap();
    let denied = host
        .call("echo.setting", json!({"key": "ui.theme"}))
        .await
        .unwrap_err();
    assert_eq!(denied.kind, ErrorKind::PermissionDenied);
    assert_eq!(
        denied.user_message,
        "The echo module isn't allowed to read the setting `ui.theme`."
    );
    let wrong = host.call("echo.say", json!({})).await.unwrap_err();
    assert_eq!(wrong.kind, ErrorKind::InvalidInput);

    host.set_enabled(&[]).await;
    let report = until(host, "echo", |r| r.state == ReportState::Disabled).await;
    assert!(!report.enabled);
    let log = setup.fake.log();
    assert_eq!(
        setup.fake.types(),
        ["module.started", "module.event", "module.stopped"]
    );
    assert_eq!(
        log[1].1, "module:echo",
        "a module's own event has it as actor"
    );
    assert_eq!(
        log[1].2,
        json!({"module": "echo", "name": "said", "data": {"text": "hi"}})
    );
    assert_eq!(log[2].2["reason"], "disabled");
    assert_eq!(log[0].1, "system");
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn a_killed_process_module_restarts_and_both_are_recorded() {
    let setup = setup(Builtins::new());
    process_echo(&setup.modules);
    let host = &setup.host;
    host.set_enabled(&["echo".into()]).await;
    let first = until(host, "echo", running).await;
    let pid = first.pid.expect("a process has a pid");
    let pid = rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()).unwrap();
    rustix::process::kill_process(pid, rustix::process::Signal::KILL).unwrap();

    let again = until(host, "echo", |r| running(r) && r.restarts == 1).await;
    assert_ne!(again.pid, first.pid);
    assert_eq!(
        host.call("echo.say", json!({"text": "back"}))
            .await
            .unwrap(),
        json!({"text": "> back"})
    );
    let log = setup.fake.log();
    assert_eq!(
        setup.fake.types(),
        ["module.started", "module.crashed", "module.started"]
    );
    assert_eq!(log[1].2["signal"], 9);
    assert_eq!(log[1].2["gave_up"], false);
    assert_eq!(log[1].2["restarts"], 0);
    host.stop("priyad is stopping").await;
    assert_eq!(setup.fake.log()[3].2["reason"], "priyad is stopping");
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn a_module_that_keeps_failing_is_given_up_on_after_six() {
    let setup = setup(Builtins::new());
    let dir = manifest(
        &setup.modules,
        "flaky",
        "kind = \"process\"\nexec = \"run\"\n",
    );
    let script = dir.join("run");
    fs::write(
        &script,
        "#!/bin/sh\necho 'something is wrong' >&2\nexit 3\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let host = &setup.host;
    host.set_enabled(&["flaky".into()]).await;
    let report = until(host, "flaky", |r| r.state == ReportState::Failed).await;
    assert_eq!(report.restarts, 5);

    let crashes: Vec<Value> = setup.fake.log().into_iter().map(|(_, _, p)| p).collect();
    assert_eq!(crashes.len(), 6, "{crashes:?}");
    assert!(crashes[..5].iter().all(|c| c["gave_up"] == false));
    let last = &crashes[5];
    assert_eq!(last["gave_up"], true);
    assert_eq!(last["exit_code"], 3);
    assert_eq!(last["stderr_tail"], "something is wrong");
    assert_eq!(last["error"], "it stopped before saying who it is");

    // Calls say it isn't running; enabling it again starts afresh.
    let error = host.call("flaky.x", json!({})).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotFound, "it offers no methods");
    host.retry("flaky").await;
    until(host, "flaky", |r| r.state != ReportState::Failed).await;
}

/// A builtin that answers `initialize` with methods its manifest doesn't
/// list, or not at all, or never answers pings: it talks raw JSON-RPC.
fn raw_builtin(
    inbound: mpsc::Receiver<Inbound>,
    outbound: mpsc::Sender<Vec<u8>>,
    behaviour: fn(&str) -> Option<Value>,
) -> BoxFuture<Ended> {
    Box::pin(async move {
        let (_peer, task) = Peer::spawn(inbound, outbound, move |_| {
            move |method: String, _: Option<Value>| async move {
                match behaviour(&method) {
                    Some(answer) => Ok(answer),
                    None => {
                        // Hang.
                        std::future::pending::<()>().await;
                        Err(RpcError::internal("unreachable"))
                    }
                }
            }
        });
        task.await.unwrap_or(Ended::Closed)
    })
}

fn liar(inbound: mpsc::Receiver<Inbound>, outbound: mpsc::Sender<Vec<u8>>) -> BoxFuture<Ended> {
    raw_builtin(inbound, outbound, |method| match method {
        "initialize" => Some(
            serde_json::to_value(InitializeResult::v1("liar", "0.1.0", ["liar.b"]))
                .expect("works in a test"),
        ),
        _ => Some(json!({})),
    })
}

fn sleepy(inbound: mpsc::Receiver<Inbound>, outbound: mpsc::Sender<Vec<u8>>) -> BoxFuture<Ended> {
    raw_builtin(inbound, outbound, |method| match method {
        "initialize" => Some(
            serde_json::to_value(InitializeResult::v1("sleepy", "0.1.0", ["sleepy.wait"]))
                .expect("works in a test"),
        ),
        "ping" => Some(json!({})),
        _ => None,
    })
}

fn deaf(inbound: mpsc::Receiver<Inbound>, outbound: mpsc::Sender<Vec<u8>>) -> BoxFuture<Ended> {
    raw_builtin(inbound, outbound, |method| match method {
        "initialize" => Some(
            serde_json::to_value(InitializeResult::v1("deaf", "0.1.0", Vec::<String>::new()))
                .expect("works in a test"),
        ),
        _ => None,
    })
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn a_module_that_lies_in_its_handshake_is_refused_at_once() {
    let setup = setup(Builtins::from([(
        "liar",
        liar as priya_module_sdk::BuiltinMain,
    )]));
    manifest(
        &setup.modules,
        "liar",
        "kind = \"builtin\"\n[provides]\nmethods = [\"liar.a\"]\n",
    );
    setup.host.set_enabled(&["liar".into()]).await;
    let report = until(&setup.host, "liar", |r| r.state == ReportState::Failed).await;
    let problem = report.problem.unwrap();
    assert!(
        problem.contains("missing: liar.a; not listed: liar.b"),
        "{problem}"
    );
    let log = setup.fake.log();
    assert_eq!(setup.fake.types(), ["module.crashed"], "never started");
    assert_eq!(log[0].2["gave_up"], true);
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn a_module_that_stops_answering_pings_is_restarted_and_calls_time_out() {
    let builtins = Builtins::from([
        ("sleepy", sleepy as priya_module_sdk::BuiltinMain),
        ("deaf", deaf as priya_module_sdk::BuiltinMain),
    ]);
    let (mut timings, policy) = timings();
    timings.ping_every = Duration::from_millis(100);
    let setup = setup_with(builtins, timings, policy);
    manifest(
        &setup.modules,
        "sleepy",
        "kind = \"builtin\"\n[provides]\nmethods = [\"sleepy.wait\"]\n",
    );
    manifest(&setup.modules, "deaf", "kind = \"builtin\"\n");
    let host = &setup.host;
    host.set_enabled(&["sleepy".into(), "deaf".into()]).await;

    // One that answers pings but not its method: the call times out.
    until(host, "sleepy", running).await;
    let error = host.call("sleepy.wait", json!({})).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Timeout);
    assert_eq!(
        error.user_message,
        "The sleepy module didn't answer within 2 s."
    );
    assert_eq!(until(host, "sleepy", running).await.restarts, 0);

    // One that doesn't answer pings is ended and started again.
    let report = until(host, "deaf", |r| r.restarts >= 1).await;
    assert!(
        report
            .problem
            .unwrap()
            .contains("didn't answer a ping within 300 ms")
    );
    let crashed = setup
        .fake
        .log()
        .into_iter()
        .find(|(t, _, _)| t == "module.crashed")
        .unwrap();
    assert_eq!(crashed.2["module"], "deaf");
    assert!(crashed.2["error"].as_str().unwrap().contains("ping"));
    host.stop("test over").await;
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn calls_go_only_to_running_modules_that_offer_the_method() {
    let setup = setup(echo_builtins());
    builtin_echo(&setup.modules);
    let host = &setup.host;
    // Found but not enabled: listed as disabled, calls refused.
    assert!(host.exists("echo").await);
    let report = host.list().await.remove(0);
    assert_eq!(
        (report.state, report.enabled),
        (ReportState::Disabled, false)
    );
    assert_eq!(report.methods, ["echo.say", "echo.emit", "echo.setting"]);
    let error = host
        .call("echo.say", json!({"text": "hi"}))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Unavailable);
    let error = host.call("echo.shout", json!({})).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotFound);

    // Enabled but not installed, and installed but broken, are listed too.
    manifest(&setup.modules, "broken", "kind = \"builtin\"\n[nonsense]\n");
    host.set_enabled(&["echo".into(), "ghost".into()]).await;
    let reports = host.list().await;
    let state = |name: &str| reports.iter().find(|r| r.name == name).map(|r| r.state);
    assert_eq!(state("ghost"), Some(ReportState::Missing));
    assert_eq!(state("broken"), Some(ReportState::Broken));
    assert!(!host.exists("ghost").await);
    // A builtin priyad doesn't have can't run.
    manifest(&setup.modules, "absent", "kind = \"builtin\"\n");
    host.set_enabled(&["absent".into()]).await;
    let report = until(host, "absent", |r| r.state == ReportState::Failed).await;
    assert!(
        report
            .problem
            .unwrap()
            .contains("no builtin module named `absent`")
    );
}

/// A module that keeps the events it is handed.
#[derive(Default)]
struct Listener;

static HEARD: Mutex<Vec<i64>> = Mutex::new(Vec::new());

impl priya_module_sdk::Module for Listener {
    async fn initialize(
        &self,
        _: &priya_module_sdk::HostClient,
        _: priya_module_sdk::InitializeParams,
    ) -> Result<InitializeResult, ApiError> {
        Ok(InitializeResult::v1(
            "listener",
            "0.1.0",
            Vec::<String>::new(),
        ))
    }

    async fn handle(
        &self,
        _: &priya_module_sdk::HostClient,
        method: &str,
        _: Value,
    ) -> Result<Value, ApiError> {
        Err(priya_module_sdk::no_such_method(method))
    }

    async fn event(&self, _: &priya_module_sdk::HostClient, event: StoredEvent) {
        HEARD.lock().expect("not poisoned").push(event.seq);
    }
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn subscribed_events_are_handed_to_the_module_in_order() {
    let setup = setup(Builtins::from([(
        "listener",
        priya_module_sdk::builtin::<Listener> as priya_module_sdk::BuiltinMain,
    )]));
    manifest(
        &setup.modules,
        "listener",
        "kind = \"builtin\"\n[subscribes]\nevents = [\"setting.*\"]\n",
    );
    setup.host.set_enabled(&["listener".into()]).await;
    until(&setup.host, "listener", running).await;
    let feed = setup
        .fake
        .feed
        .lock()
        .unwrap()
        .pop()
        .expect("it subscribed");
    for seq in 1..=3 {
        let event: StoredEvent = serde_json::from_value(json!({
            "seq": seq, "prev_hash": "", "hash": "",
            "event": {"id": "01J00000000000000000000000", "ts": "2026-10-20T00:00:00Z",
                      "type": "setting.changed", "version": 1, "stream": "settings",
                      "correlation": null, "causation": null, "actor": "user", "payload": {}}
        }))
        .unwrap();
        feed.send(event).await.unwrap();
    }
    let heard = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let heard = HEARD.lock().expect("not poisoned").clone();
            if heard.len() == 3 {
                return heard;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(heard, [1, 2, 3]);
    assert_eq!(DeliverEvent::NAME, "event");
}

/// A builtin that answers `initialize` and `ping` but never `shutdown`.
fn stubborn(inbound: mpsc::Receiver<Inbound>, outbound: mpsc::Sender<Vec<u8>>) -> BoxFuture<Ended> {
    raw_builtin(inbound, outbound, |method| match method {
        "initialize" => Some(
            serde_json::to_value(InitializeResult::v1(
                "stubborn",
                "0.1.0",
                Vec::<String>::new(),
            ))
            .expect("works in a test"),
        ),
        "ping" => Some(json!({})),
        _ => None,
    })
}

/// A builtin that never answers anything.
fn mute(inbound: mpsc::Receiver<Inbound>, outbound: mpsc::Sender<Vec<u8>>) -> BoxFuture<Ended> {
    raw_builtin(inbound, outbound, |_| None)
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn turning_a_module_on_while_it_stops_waits_for_the_old_copy() {
    let setup = setup(Builtins::from([(
        "stubborn",
        stubborn as priya_module_sdk::BuiltinMain,
    )]));
    manifest(&setup.modules, "stubborn", "kind = \"builtin\"\n");
    let host = Arc::new(setup.host);
    host.set_enabled(&["stubborn".into()]).await;
    until(&host, "stubborn", running).await;
    // Off, and on again while it is still being stopped (it ignores
    // `shutdown`, so stopping takes the grace time).
    let off = tokio::spawn({
        let host = host.clone();
        async move { host.set_enabled(&[]).await }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    host.set_enabled(&["stubborn".into()]).await;
    off.await.expect("stops");
    until(&host, "stubborn", running).await;
    assert_eq!(
        setup.fake.types(),
        ["module.started", "module.stopped", "module.started"]
    );
    host.stop("test over").await;
    // Once stopped, nothing starts again.
    host.set_enabled(&["stubborn".into()]).await;
    host.retry("stubborn").await;
    assert_eq!(host.running().await, Vec::<String>::new());
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn a_process_that_ignores_shutdown_and_sigterm_is_killed() {
    let (mut timings, policy) = timings();
    timings.ping_every = Duration::from_secs(60);
    timings.stop_grace = Duration::from_millis(300);
    let setup = setup_with(Builtins::new(), timings, policy);
    let dir = manifest(
        &setup.modules,
        "stubborn",
        "kind = \"process\"\nexec = \"run\"\n",
    );
    // It says who it is (priyad's first request has id 1), then ignores
    // everything, SIGTERM included, with a child of its own.
    let hello = r#"{"jsonrpc":"2.0","id":1,"result":{"api":"v1","name":"stubborn","version":"0.1.0","methods":[]}}"#;
    let script = format!(
        "#!/bin/sh\ntrap '' TERM\nprintf 'Content-Length: {}\\r\\n\\r\\n%s' '{hello}'\nsleep 600 &\nwhile true; do sleep 1; done\n",
        hello.len()
    );
    fs::write(dir.join("run"), script).expect("works in a test");
    fs::set_permissions(dir.join("run"), fs::Permissions::from_mode(0o755))
        .expect("works in a test");
    let host = &setup.host;
    host.set_enabled(&["stubborn".into()]).await;
    let pid = until(host, "stubborn", running).await.pid.expect("a pid");
    let started = std::time::Instant::now();
    host.set_enabled(&[]).await;
    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(600),
        "shutdown, then SIGTERM, then SIGKILL: {took:?}"
    );
    let group = rustix::process::Pid::from_raw(i32::try_from(pid).expect("small")).expect("a pid");
    assert!(
        rustix::process::test_kill_process_group(group).is_err(),
        "it and its child are gone"
    );
    assert_eq!(setup.fake.types(), ["module.started", "module.stopped"]);
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn a_module_that_never_says_who_it_is_fails_after_the_handshake_time() {
    let (mut timings, policy) = timings();
    timings.handshake = Duration::from_millis(200);
    let setup = setup_with(
        Builtins::from([("mute", mute as priya_module_sdk::BuiltinMain)]),
        timings,
        policy,
    );
    manifest(&setup.modules, "mute", "kind = \"builtin\"\n");
    setup.host.set_enabled(&["mute".into()]).await;
    let report = until(&setup.host, "mute", |r| r.restarts >= 1).await;
    assert_eq!(
        report.problem.as_deref(),
        Some("it didn't say who it is within 200 ms")
    );
    setup.host.stop("test over").await;
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn a_running_module_outlives_its_folder_but_is_not_restarted_without_it() {
    let (timings, mut policy) = timings();
    policy.first = Duration::from_millis(500);
    let setup = setup_with(Builtins::new(), timings, policy);
    process_echo(&setup.modules);
    let host = &setup.host;
    host.set_enabled(&["echo".into()]).await;
    let pid = until(host, "echo", running).await.pid.expect("a pid");
    fs::remove_dir_all(setup.modules.join("echo")).expect("works in a test");
    // Still listed and answering.
    let report = until(host, "echo", running).await;
    assert_eq!(report.version.as_deref(), Some("0.1.0"));
    assert_eq!(
        host.call("echo.say", json!({"text": "hi"})).await.unwrap(),
        json!({"text": "> hi"})
    );
    let too_big = host
        .call("echo.say", json!({"text": "x".repeat(17 * 1024 * 1024)}))
        .await;
    assert_eq!(too_big.unwrap_err().kind, ErrorKind::InvalidInput);
    // Once it fails, it isn't started from nowhere.
    let pid = rustix::process::Pid::from_raw(i32::try_from(pid).expect("small")).expect("a pid");
    rustix::process::kill_process(pid, rustix::process::Signal::KILL).expect("killed");
    let report = until(host, "echo", |r| r.state == ReportState::Failed).await;
    assert_eq!(report.problem.as_deref(), Some("its folder is gone"));
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn a_host_dropped_without_stopping_still_stops_its_modules() {
    let setup = setup(echo_builtins());
    builtin_echo(&setup.modules);
    setup.host.set_enabled(&["echo".into()]).await;
    until(&setup.host, "echo", running).await;
    let fake = setup.fake.clone();
    drop(setup.host);
    for _ in 0..500 {
        if fake.types().contains(&"module.stopped".to_owned()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let log = fake.log();
    assert_eq!(fake.types(), ["module.started", "module.stopped"]);
    assert_eq!(log[1].2["reason"], "priyad is stopping");
}
