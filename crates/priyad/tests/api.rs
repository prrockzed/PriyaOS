//! The Phase 0 methods, served on a socket over a daemon's parts in
//! temporary folders (0.7).

mod common;

use std::collections::VecDeque;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use common::{Machine, machine};
use priya_api::methods::ALL;
use priyad::api::{MAX_PATTERN_LEN, MAX_PATTERNS, MAX_SUBSCRIPTIONS, router};
use priyad::daemon::own_uid;
use priyad::rpc::server::serve;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::watch;

/// A client speaking raw JSON-RPC lines.
struct Client {
    lines: Lines<BufReader<OwnedReadHalf>>,
    write: OwnedWriteHalf,
    next_id: u64,
    /// Notifications that came while waiting for a reply.
    waiting: VecDeque<Value>,
}

impl Client {
    async fn read(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(5), self.lines.next_line())
            .await
            .expect("a line in time")
            .expect("the socket reads")
            .expect("the connection is open");
        serde_json::from_str(&line).expect("JSON")
    }

    /// The whole response to `method`.
    async fn call(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let mut line = serde_json::to_vec(&request).expect("JSON");
        line.push(b'\n');
        self.write
            .write_all(&line)
            .await
            .expect("the socket writes");
        loop {
            let message = self.read().await;
            if message["id"] == id {
                return message;
            }
            self.waiting.push_back(message);
        }
    }

    /// The result of `method`, which must succeed.
    async fn ok(&mut self, method: &str, params: Value) -> Value {
        let response = self.call(method, params).await;
        assert!(response.get("error").is_none(), "{method}: {response}");
        response["result"].clone()
    }

    /// The error kind and message of `method`, which must fail.
    async fn refused(&mut self, method: &str, params: Value) -> (String, String) {
        let response = self.call(method, params).await;
        let data = &response["error"]["data"];
        let kind = data["kind"]
            .as_str()
            .unwrap_or_else(|| panic!("{response}"));
        (
            kind.to_owned(),
            data["user_message"].as_str().expect("a message").to_owned(),
        )
    }

    async fn notification(&mut self) -> Value {
        match self.waiting.pop_front() {
            Some(waiting) => waiting,
            None => self.read().await,
        }
    }
}

/// Serves the machine's daemon; dropping the sender stops it.
async fn served(m: &mut Machine) -> (Client, watch::Sender<bool>) {
    let listener = m.listener.take().expect("not served yet");
    let (stop, stopping) = watch::channel(false);
    tokio::spawn(serve(
        listener,
        own_uid(),
        Arc::new(router()),
        m.daemon.clone(),
        stopping,
    ));
    let stream = UnixStream::connect(&m.daemon.doctor.runtime.socket)
        .await
        .expect("connects");
    let (read, write) = stream.into_split();
    let client = Client {
        lines: BufReader::new(read).lines(),
        write,
        next_id: 0,
        waiting: VecDeque::new(),
    };
    (client, stop)
}

// covers: CORE-03, CORE-04
// suite: integration
#[tokio::test]
async fn settings_get_set_and_unset_over_the_socket() {
    let mut m = machine().await;
    let (mut c, _stop) = served(&mut m).await;

    // A real setting with no value is null, not "not found".
    let unset = c.ok("v1.settings.get", json!({"key": "ai.strategy"})).await;
    assert_eq!(unset["value"], Value::Null);
    let (kind, message) = c
        .refused("v1.settings.get", json!({"key": "ui.themes"}))
        .await;
    assert_eq!(kind, "not_found");
    assert!(
        message.starts_with("`ui.themes` isn't a setting."),
        "{message}"
    );
    let (kind, message) = c.refused("v1.settings.get", json!({"key": "ui..x"})).await;
    assert_eq!(kind, "invalid_input");
    assert_eq!(
        message,
        "`ui..x` isn't a valid settings key; write one like ui.theme."
    );

    c.ok(
        "v1.settings.set",
        json!({"key": "ui.theme", "value": "light"}),
    )
    .await;
    let explained = c
        .ok(
            "v1.settings.get",
            json!({"key": "ui.theme", "explain": true}),
        )
        .await;
    assert_eq!(explained["value"], "light");
    assert_eq!(explained["origins"][0]["layer"], "user");
    assert!(
        fs::read_to_string(&m.user_settings)
            .expect("written")
            .contains("light")
    );

    for made_up in ["ui.themes", "ui.theme.bogus"] {
        let (kind, _) = c
            .refused("v1.settings.unset", json!({"key": made_up}))
            .await;
        assert_eq!(kind, "not_found", "{made_up} isn't quietly accepted");
        let (kind, _) = c.refused("v1.settings.get", json!({"key": made_up})).await;
        assert_eq!(kind, "not_found", "{made_up}");
    }
    c.ok("v1.settings.unset", json!({"key": "ui.theme"})).await;
    let back = c.ok("v1.settings.get", json!({"key": "ui.theme"})).await;
    assert_eq!(back["value"], "dark", "the default again");
}

// covers: CORE-04
// suite: integration
#[tokio::test]
async fn status_methods_and_audit_answer() {
    let mut m = machine().await;
    let (mut c, _stop) = served(&mut m).await;
    c.ok(
        "v1.settings.set",
        json!({"key": "ui.theme", "value": "light"}),
    )
    .await;

    let status = c.ok("v1.system.status", json!({})).await;
    assert_eq!(status["event_count"], 1, "the setting change");
    assert_eq!(status["version"], env!("CARGO_PKG_VERSION"));

    let methods = c.ok("v1.system.methods", json!({})).await;
    let mut listed: Vec<&str> = methods["methods"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|method| method["name"].as_str().expect("a name"))
        .collect();
    listed.sort_unstable();
    let served: Vec<&str> = router().names().collect();
    assert_eq!(listed, served, "exactly what is served");
    assert_eq!(listed.len(), ALL.len(), "Phase 0 serves every method");

    let report = c.ok("v1.audit.verify", json!({})).await;
    assert_eq!(
        (report["ok"].clone(), report["checked"].clone()),
        (json!(true), json!(1))
    );
    for range in [
        json!({"from": 0}),
        json!({"to": 0}),
        json!({"from": 5, "to": 2}),
    ] {
        let (kind, _) = c.refused("v1.audit.verify", range.clone()).await;
        assert_eq!(kind, "invalid_input", "{range}");
    }
}

// covers: CORE-02, CORE-04
// suite: integration
#[tokio::test]
async fn subscriptions_stream_full_events_until_unsubscribed() {
    let mut m = machine().await;
    let (mut c, _stop) = served(&mut m).await;
    c.ok(
        "v1.settings.set",
        json!({"key": "ui.theme", "value": "light"}),
    )
    .await;

    let subscription =
        c.ok("v1.events.subscribe", json!({"from_seq": 0})).await["subscription"].clone();
    let event = c.notification().await;
    assert_eq!(event["method"], "v1.events.event");
    assert_eq!(event["params"]["subscription"], subscription);
    assert_eq!(event["params"]["event"]["seq"], 1);
    let envelope = &event["params"]["event"]["event"];
    assert_eq!(envelope["type"], "setting.changed");
    assert_eq!(envelope["payload"]["key"], "ui.theme", "the full payload");

    c.ok(
        "v1.events.unsubscribe",
        json!({"subscription": subscription}),
    )
    .await;
    let (kind, _) = c
        .refused(
            "v1.events.unsubscribe",
            json!({"subscription": subscription}),
        )
        .await;
    assert_eq!(kind, "not_found");
    // Nothing more for it after the unsubscribe reply.
    c.ok(
        "v1.settings.set",
        json!({"key": "ui.theme", "value": "dark"}),
    )
    .await;
    c.ok("v1.system.status", json!({})).await;
    assert!(c.waiting.is_empty(), "{:?}", c.waiting);
}

// covers: CORE-04
// suite: integration
#[tokio::test]
async fn subscriptions_are_limited() {
    let mut m = machine().await;
    let (mut c, _stop) = served(&mut m).await;
    let too_many_types: Vec<String> = (0..=MAX_PATTERNS).map(|i| format!("t{i}")).collect();
    for params in [
        json!({"types": too_many_types}),
        json!({"types": ["x".repeat(MAX_PATTERN_LEN + 1)]}),
        json!({"stream_prefix": "x".repeat(MAX_PATTERN_LEN + 1)}),
        json!({"from_seq": -1}),
    ] {
        let (kind, _) = c.refused("v1.events.subscribe", params).await;
        assert_eq!(kind, "invalid_input");
    }
    for _ in 0..MAX_SUBSCRIPTIONS {
        c.ok("v1.events.subscribe", json!({})).await;
    }
    let (kind, message) = c.refused("v1.events.subscribe", json!({})).await;
    assert_eq!(kind, "invalid_input");
    assert!(message.contains("stop one first"), "{message}");
}

// covers: CORE-01, CORE-04
// suite: integration
#[tokio::test]
async fn a_stream_ends_with_an_error_at_an_unreadable_event() {
    let mut m = machine().await;
    let (mut c, _stop) = served(&mut m).await;
    // Over 8 KiB, so the payload goes to a blob file.
    let long = "x".repeat(10 * 1024);
    c.ok(
        "v1.settings.set",
        json!({"key": "modules.weather.notes", "value": long}),
    )
    .await;
    let blobs = walk(&m.daemon.doctor.data_dir.join("blobs"));
    assert_eq!(blobs.len(), 1, "one blob was written");
    fs::remove_file(&blobs[0]).expect("removed");

    let subscription =
        c.ok("v1.events.subscribe", json!({"from_seq": 0})).await["subscription"].clone();
    let ended = c.notification().await;
    assert_eq!(ended["method"], "v1.events.ended", "{ended}");
    assert_eq!(ended["params"]["subscription"], subscription);
    assert_eq!(ended["params"]["reason"], "error");
    assert!(
        ended["params"]["message"]
            .as_str()
            .expect("a message")
            .starts_with("Event 1 couldn't be read"),
        "{ended}"
    );
    // It is forgotten: unsubscribing finds nothing.
    let (kind, _) = c
        .refused(
            "v1.events.unsubscribe",
            json!({"subscription": subscription}),
        )
        .await;
    assert_eq!(kind, "not_found");
}

/// Every file under `dir`.
fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

// covers: CORE-03, CORE-04, CORE-05
// suite: integration
#[tokio::test]
async fn the_other_read_methods_answer() {
    let mut m = machine().await;
    let (mut c, _stop) = served(&mut m).await;

    // Everything at once, with no key.
    let all = c.ok("v1.settings.get", json!({})).await;
    assert_eq!(all["value"]["ui"]["theme"], "dark");
    let schema = c.ok("v1.settings.schema", json!({})).await;
    assert!(schema["schema"]["properties"]["ui"].is_object());

    // The model in use shows in the status once one is chosen.
    let provider = json!({"kind": "anthropic"});
    c.ok(
        "v1.settings.set",
        json!({"key": "ai.providers.anthropic", "value": provider}),
    )
    .await;
    c.ok(
        "v1.settings.set",
        json!({"key": "ai.strategy", "value": "fixed:anthropic/claude-x"}),
    )
    .await;
    let status = c.ok("v1.system.status", json!({})).await;
    assert_eq!(
        status["model"],
        json!({"provider": "anthropic", "model": "claude-x"})
    );

    let report = c.ok("v1.system.doctor", json!({})).await;
    let ids: Vec<&str> = report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .map(|c| c["id"].as_str().expect("an id"))
        .collect();
    assert!(ids.contains(&"store.integrity"), "{ids:?}");
}

// covers: CORE-01
// suite: integration
#[tokio::test]
async fn audit_verify_names_the_changed_event() {
    let mut m = machine().await;
    let (mut c, _stop) = served(&mut m).await;
    c.ok(
        "v1.settings.set",
        json!({"key": "ui.theme", "value": "light"}),
    )
    .await;
    let db = rusqlite::Connection::open(&m.daemon.db_path).expect("opens");
    db.execute_batch(
        "DROP TRIGGER IF EXISTS events_no_update;
         UPDATE events SET payload = json_set(payload, '$.new', 'dark') WHERE seq = 1;",
    )
    .expect("tampered");
    let report = c.ok("v1.audit.verify", json!({})).await;
    assert_eq!(report["ok"], false);
    assert_eq!(report["first_bad_seq"], 1);
    assert_eq!(report["reason"], "hash_mismatch");
}
