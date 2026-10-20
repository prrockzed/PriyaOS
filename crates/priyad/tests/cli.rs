//! The `priya` CLI against the real priyad, each in a temporary home (0.7).
//!
//! Needs both binaries built: `cargo test --workspace` builds them, or
//! `cargo build -p priyad -p priya-cli` before `cargo test -p priyad`.

use std::io::{BufRead, BufReader};
use std::process::{Child, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use priya_api::methods::ALL;
use priya_testkit::{TestDaemon, workspace_binary};
use serde_json::Value;

fn daemon() -> TestDaemon {
    TestDaemon::start(workspace_binary("priyad")).expect("priyad starts")
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

/// `priya events tail --json` running in the background.
struct Tail {
    child: Child,
    lines: mpsc::Receiver<String>,
}

impl Tail {
    fn start(daemon: &TestDaemon, args: &[&str]) -> Self {
        let mut child = daemon
            .command(workspace_binary("priya"))
            .args(["--json", "events", "tail"])
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("priya runs");
        let stdout = child.stdout.take().expect("piped");
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self { child, lines }
    }

    /// The next `count` events.
    fn events(&self, count: usize) -> Vec<Value> {
        (0..count)
            .map(|_| {
                let line = self
                    .lines
                    .recv_timeout(Duration::from_secs(5))
                    .expect("an event in time");
                serde_json::from_str(&line).expect("one JSON event per line")
            })
            .collect()
    }
}

impl Drop for Tail {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn seqs_and_types(events: &[Value]) -> Vec<(i64, String)> {
    events
        .iter()
        .map(|e| {
            (
                e["seq"].as_i64().expect("a seq"),
                e["event"]["type"].as_str().expect("a type").to_owned(),
            )
        })
        .collect()
}

// covers: CORE-03
// suite: integration
// acceptance: 0.7
#[test]
fn settings_set_via_cli_emits_event_and_persists_across_restart() {
    let mut d = daemon();
    assert_eq!(
        ok(&d, &["settings", "set", "ui.theme", "light"]),
        "Set ui.theme.\n"
    );
    let tail = Tail::start(&d, &["--from", "0", "--type", "setting.*"]);
    let changed = &tail.events(1)[0];
    assert_eq!(changed["event"]["payload"]["key"], "ui.theme");
    assert_eq!(changed["event"]["payload"]["new"], "light");
    drop(tail);

    d.restart().expect("priyad restarts");
    assert_eq!(ok(&d, &["settings", "get", "ui.theme"]), "light\n");
}

// covers: CORE-03
// suite: integration
// acceptance: 0.7
#[test]
fn settings_file_edit_visible_via_api_within_1s() {
    let d = daemon();
    std::fs::write(d.user_settings(), "[ui]\ntheme = \"light\"\n").expect("written");
    let start = Instant::now();
    while ok(&d, &["settings", "get", "ui.theme"]) != "light\n" {
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "not seen in time; log:\n{}",
            d.log()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

// covers: CORE-01
// suite: integration
// acceptance: 0.7
#[test]
fn events_tail_streams_and_resumes_from_seq() {
    let d = daemon();
    ok(&d, &["settings", "set", "ui.theme", "light"]);
    let tail = Tail::start(&d, &["--from", "0"]);
    assert_eq!(
        seqs_and_types(&tail.events(2)),
        [
            (1, "daemon.started".to_owned()),
            (2, "setting.changed".to_owned())
        ]
    );
    // New events arrive as they happen.
    ok(&d, &["settings", "set", "ui.theme", "dark"]);
    assert_eq!(tail.events(1)[0]["seq"], 3);
    drop(tail);

    // Resuming after event 2 starts at 3.
    let resumed = Tail::start(&d, &["--from", "2"]);
    assert_eq!(resumed.events(1)[0]["seq"], 3);
}

// covers: CORE-01
// suite: integration
// acceptance: 0.7
#[test]
fn audit_verify_passes_then_detects_tampering() {
    let mut d = daemon();
    ok(&d, &["settings", "set", "ui.theme", "light"]);
    assert_eq!(
        ok(&d, &["audit", "verify"]),
        "The history is intact: 2 events checked.\n"
    );

    d.stop().expect("priyad stops");
    let db = rusqlite::Connection::open(d.home().paths().data.join("priya.db")).expect("opens");
    // As someone editing the file would: without the append-only guards.
    db.execute_batch(
        "DROP TRIGGER IF EXISTS events_no_update;
         UPDATE events SET payload = json_set(payload, '$.new', 'dark') WHERE seq = 2;",
    )
    .expect("tampered");
    drop(db);
    d.restart().expect("priyad restarts");

    let output = priya(&d, &["audit", "verify"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "Event 2 was changed after it was written."
    );
}

// covers: CORE-04
// suite: integration
#[tokio::test]
async fn the_test_daemon_hands_out_a_typed_client() {
    let d = daemon();
    let mut client = d.client().await.expect("connects");
    let status = client
        .call::<priya_api::methods::system::Status>(&priya_api::methods::NoParams {})
        .await
        .expect("answers");
    assert_eq!(status.version, env!("CARGO_PKG_VERSION"));
    assert!(d.pid().is_some());
}

// covers: CORE-04
// suite: integration
// acceptance: 0.7
#[test]
fn second_instance_refuses_to_start() {
    let d = daemon();
    let second = d
        .command(workspace_binary("priyad"))
        .output()
        .expect("runs");
    assert!(!second.status.success());
    assert!(
        String::from_utf8_lossy(&second.stderr).contains("already running"),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    // The first one is unharmed.
    ok(&d, &["status"]);
}

// covers: CORE-01
// suite: integration
// acceptance: 0.7
#[test]
fn sigterm_shuts_down_cleanly_and_chain_verifies() {
    let mut d = daemon();
    // Following from the start, so it is surely subscribed once the first
    // event arrives.
    let tail = Tail::start(&d, &["--from", "0"]);
    tail.events(1);
    let status = d.stop().expect("priyad stops");
    assert!(status.success(), "{status}");
    assert!(!d.socket().exists(), "the socket is removed");
    // The stream was told it ended, and the CLI says so.
    let output = tail_output(tail);
    assert_eq!(output.status.code(), Some(1));

    d.restart().expect("priyad restarts");
    let report: Value =
        serde_json::from_str(&ok(&d, &["--json", "audit", "verify"])).expect("JSON");
    assert_eq!(report["ok"], true);
    let tail = Tail::start(&d, &["--from", "0"]);
    let types: Vec<String> = seqs_and_types(&tail.events(3))
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    assert_eq!(
        types,
        ["daemon.started", "daemon.stopped", "daemon.started"]
    );
}

/// Waits for a tail to end by itself.
fn tail_output(mut tail: Tail) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = tail.child.try_wait().expect("waits") {
            return Output {
                status,
                stdout: Vec::new(),
                stderr: Vec::new(),
            };
        }
        assert!(Instant::now() < deadline, "the tail didn't end");
        std::thread::sleep(Duration::from_millis(20));
    }
}

// covers: CORE-04
// suite: contract
// acceptance: 0.7
#[test]
fn cli_json_output_matches_result_schema() {
    let d = daemon();
    ok(&d, &["settings", "set", "ui.theme", "light"]);
    let read_commands: &[(&[&str], &str)] = &[
        (&["status"], "v1.system.status"),
        (&["settings", "get"], "v1.settings.get"),
        (
            &["settings", "get", "ui.theme", "--explain"],
            "v1.settings.get",
        ),
        (&["settings", "schema"], "v1.settings.schema"),
        (&["audit", "verify"], "v1.audit.verify"),
        (&["api", "methods"], "v1.system.methods"),
        (&["doctor"], "v1.system.doctor"),
    ];
    for (args, method) in read_commands {
        let info = ALL
            .iter()
            .find(|m| m.name == *method)
            .expect("a known method");
        let schema = (info.result)().to_value();
        let validator = jsonschema::validator_for(&schema).expect("a valid schema");
        let mut full = vec!["--json"];
        full.extend_from_slice(args);
        let output = priya(&d, &full);
        let text = String::from_utf8(output.stdout).expect("UTF-8");
        let answer: Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("priya {args:?} printed no JSON ({e}): {text}"));
        let problems: Vec<String> = validator
            .iter_errors(&answer)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        assert!(problems.is_empty(), "priya {args:?}: {problems:#?}");
    }
}

// covers: CORE-04
// suite: integration
#[test]
fn systemd_hears_ready_then_stopping() {
    // A daemon of its own, so it can report to a socket of the test's.
    let mut d = daemon();
    d.stop().expect("the first run stops");
    let dir = tempfile::tempdir().expect("a temporary folder");
    let notify = dir.path().join("notify");
    let listener = std::os::unix::net::UnixDatagram::bind(&notify).expect("binds");
    listener
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("a timeout");
    let mut priyad = d
        .command(workspace_binary("priyad"))
        .env("NOTIFY_SOCKET", &notify)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("starts");
    let next = || {
        let mut buffer = [0_u8; 64];
        let n = listener.recv(&mut buffer).expect("a message in time");
        String::from_utf8_lossy(&buffer[..n]).into_owned()
    };
    assert_eq!(next(), "READY=1");
    ok(&d, &["status"]);
    rustix::process::kill_process(
        rustix::process::Pid::from_child(&priyad),
        rustix::process::Signal::TERM,
    )
    .expect("signalled");
    assert_eq!(next(), "STOPPING=1");
    assert!(priyad.wait().expect("exits").success());
}
