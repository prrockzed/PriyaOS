//! mock-priyad answers from its script, as priyad would on the wire.

use std::time::Duration;

use priya_api::client::{Client, ClientError};
use priya_testkit::mock_priyad::{Script, serve};
use priya_testkit::workspace_binary;
use serde_json::{Value, json};

fn script() -> Script {
    let step = |seq: u64, kind: &str, text: Option<&str>| {
        json!({"delay_ms": 20, "method": "v1.intent.stream",
               "params": {"intent_id": "i1", "seq": seq, "kind": kind, "text": text}})
    };
    serde_json::from_value(json!({
        "on_connect": [{"delay_ms": 0, "method": "v1.hello", "params": {}}],
        "methods": {
            "v1.settings.get": [
                {"params": {"key": "ui.bar.width_px"}, "result": {"value": 720}},
                {"result": {"value": "light"}},
                {"result": {"value": "dark"}}
            ],
            "v1.settings.set": [{"result": {}, "broadcast": true, "notify": [
                {"delay_ms": 0, "method": "v1.events.event", "params": {"subscription": "sub-1", "event": {"type": "setting.changed"}}},
                {"delay_ms": 60000, "method": "v1.events.event", "params": {}}
            ]}],
            "v1.bye": [{"result": {}, "close": true}],
            "v1.intent.submit": [{
                "result": {"intent_id": "i1", "conversation_id": "c1"},
                "notify": [step(1, "started", None), step(2, "delta", Some("Hi")), step(3, "delta", Some(" there")),
                           {"delay_ms": 5000, "method": "v1.intent.stream", "params": {"intent_id": "i1", "seq": 9, "kind": "done"}}]
            }],
            "v1.intent.cancel": [{"result": {}, "stop": "v1.intent.submit", "notify": [step(4, "cancelled", None)]}],
            "v1.ai.ask": [{"error": {"kind": "provider_error", "retryable": true, "user_message": "Gemini is busy right now."}}]
        }
    }))
    .expect("a valid script")
}

async fn next(client: &mut Client) -> (String, Value) {
    let n = tokio::time::timeout(Duration::from_secs(5), client.next_notification())
        .await
        .expect("a notification in time")
        .expect("the connection works")
        .expect("still connected");
    (n.method, n.params.unwrap_or(Value::Null))
}

// covers: UX-02
// suite: unit
#[tokio::test]
async fn mock_priyad_plays_replies_notifications_and_cancels() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("priyad.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let server = tokio::spawn(async move { serve(listener, script()).await.unwrap() });
    let mut client = Client::connect(&socket).await.unwrap();

    assert_eq!(next(&mut client).await.0, "v1.hello", "sent on connect");
    // Matched by params, and not used up.
    for _ in 0..2 {
        let width = client
            .call_raw("v1.settings.get", json!({"key": "ui.bar.width_px"}))
            .await
            .unwrap();
        assert_eq!(width["value"], 720);
    }
    // Replies in order, then the last again.
    for theme in ["light", "dark", "dark"] {
        let got = client
            .call_raw("v1.settings.get", json!({"key": "ui.theme"}))
            .await
            .unwrap();
        assert_eq!(got["value"], theme);
    }
    let submitted = client
        .call_raw("v1.intent.submit", json!({"text": "hi"}))
        .await
        .unwrap();
    assert_eq!(submitted["intent_id"], "i1");
    let mut kinds = Vec::new();
    for _ in 0..3 {
        kinds.push(
            next(&mut client).await.1["kind"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    assert_eq!(kinds, ["started", "delta", "delta"]);
    // Cancel drops the long-delayed "done" and says "cancelled" instead.
    client
        .call_raw("v1.intent.cancel", json!({"intent_id": "i1"}))
        .await
        .unwrap();
    assert_eq!(next(&mut client).await.1["kind"], "cancelled");
    let quiet = tokio::time::timeout(Duration::from_millis(300), client.next_notification()).await;
    assert!(quiet.is_err(), "nothing more after a cancel");

    match client.call_raw("v1.ai.ask", json!({})).await {
        Err(ClientError::Api(error)) => assert_eq!(error.user_message, "Gemini is busy right now."),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        client.call_raw("v1.nope", json!({})).await,
        Err(ClientError::Api(_))
    ));
    server.abort();
}

// covers: UX-02
// suite: unit
#[tokio::test]
async fn mock_priyad_broadcasts_and_hangs_up_when_scripted() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("priyad.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let server = tokio::spawn(async move { serve(listener, script()).await.unwrap() });
    let mut asking = Client::connect(&socket).await.unwrap();
    let mut watching = Client::connect(&socket).await.unwrap();
    for client in [&mut asking, &mut watching] {
        assert_eq!(next(client).await.0, "v1.hello");
    }

    // Started by the other connection, and still sending (its last step
    // waits a minute) when `asking` hangs up below.
    watching
        .call_raw(
            "v1.settings.set",
            json!({"key": "ui.theme", "value": "light"}),
        )
        .await
        .unwrap();
    for client in [&mut asking, &mut watching] {
        let (method, params) = next(client).await;
        assert_eq!(method, "v1.events.event", "reaches every connection");
        assert_eq!(params["event"]["type"], "setting.changed");
    }

    asking.call_raw("v1.bye", json!({})).await.unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(5), asking.next_notification())
        .await
        .expect("hung up in time");
    assert!(matches!(closed, Ok(None)), "{closed:?}");
    // The other connection keeps working (this is just the first
    // scripted reply: the mock keeps no settings).
    let theme = watching
        .call_raw("v1.settings.get", json!({"key": "ui.theme"}))
        .await
        .unwrap();
    assert_eq!(theme["value"], "light");
    server.abort();
}

// covers: UX-02
// suite: unit
#[tokio::test]
async fn mock_priyad_answers_bad_lines_like_priyad() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("script.json");
    std::fs::write(
        &path,
        r#"{"methods": {"v1.broken": [{"error": {"not": "an api error"}}]}}"#,
    )
    .unwrap();
    let script = Script::read(&path).unwrap();
    let missing = Script::read(&dir.path().join("nope.json")).unwrap_err();
    assert!(missing.contains("nope.json"), "{missing}");
    std::fs::write(&path, r#"{"method": {}}"#).unwrap();
    assert!(Script::read(&path).is_err(), "unknown fields are refused");

    let socket = dir.path().join("priyad.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let server = tokio::spawn(async move { serve(listener, script).await.unwrap() });
    let (reader, mut writer) = tokio::net::UnixStream::connect(&socket)
        .await
        .unwrap()
        .into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut reply_to = async |line: &str| -> Value {
        writer
            .write_all(format!("{line}\n").as_bytes())
            .await
            .unwrap();
        let reply = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
            .await
            .expect("a reply in time")
            .unwrap()
            .expect("still connected");
        serde_json::from_str(&reply).unwrap()
    };
    let code = |reply: &Value| reply["error"]["code"].as_i64().unwrap();

    use priya_api::rpc::code::{INTERNAL_ERROR, INVALID_REQUEST, PARSE_ERROR};
    assert_eq!(code(&reply_to("{not json").await), PARSE_ERROR);
    let batch = r#"[{"jsonrpc": "2.0", "id": 1, "method": "v1.broken"}]"#;
    assert_eq!(code(&reply_to(batch).await), INVALID_REQUEST);
    let no_method = r#"{"jsonrpc": "2.0", "id": 1}"#;
    assert_eq!(code(&reply_to(no_method).await), INVALID_REQUEST);
    // A client notification gets no answer, so the next reply is the call's.
    let reply = reply_to(concat!(
        r#"{"jsonrpc": "2.0", "method": "v1.broken"}"#,
        "\n",
        r#"{"jsonrpc": "2.0", "id": 2, "method": "v1.broken"}"#
    ))
    .await;
    assert_eq!(reply["id"], 2, "{reply}");
    assert_eq!(
        code(&reply),
        INTERNAL_ERROR,
        "a scripted error that isn't an ApiError"
    );
    server.abort();
}

// covers: UX-02
// suite: unit
#[tokio::test]
async fn the_mock_priyad_binary_serves_a_script_file() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("priyad.sock");
    let script = dir.path().join("script.json");
    let log = dir.path().join("requests.ndjson");
    std::fs::write(&script, serde_json::to_string(&script_value()).unwrap()).unwrap();
    let child = std::process::Command::new(workspace_binary("mock-priyad"))
        .arg("--socket")
        .arg(&socket)
        .arg("--script")
        .arg(&script)
        .arg("--log")
        .arg(&log)
        .spawn()
        .unwrap();
    // Killed however the test ends, so a failed assert leaves nothing running.
    let mut child = KillOnDrop(child);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut client = loop {
        if let Ok(client) = Client::connect(&socket).await {
            break client;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "mock-priyad didn't start"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let got = client
        .call_raw("v1.system.status", json!({}))
        .await
        .unwrap();
    assert_eq!(got["version"], "mock");
    // Every request is in the log, as sent.
    let logged: Vec<Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(logged.len(), 1, "{logged:?}");
    assert_eq!(logged[0]["method"], "v1.system.status");
    // Its socket is private, and a second one won't take it over.
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let second = std::process::Command::new(workspace_binary("mock-priyad"))
        .arg("--socket")
        .arg(&socket)
        .arg("--script")
        .arg(&script)
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("already listening"));
    // SIGTERM: it stops and removes its socket.
    let pid = rustix::process::Pid::from_raw(i32::try_from(child.0.id()).unwrap()).unwrap();
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).unwrap();
    assert!(child.0.wait().unwrap().success());
    assert!(!socket.exists());

    let bad = std::process::Command::new(workspace_binary("mock-priyad"))
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    // A flag without its value, and a log it can't open, say so at once.
    let no_value = std::process::Command::new(workspace_binary("mock-priyad"))
        .args(["--socket", "s", "--script", "x", "--log"])
        .output()
        .unwrap();
    assert_eq!(no_value.status.code(), Some(2));
    let unopenable = dir.path().join("missing/requests.ndjson");
    let bad_log = std::process::Command::new(workspace_binary("mock-priyad"))
        .arg("--socket")
        .arg(&socket)
        .arg("--script")
        .arg(&script)
        .arg("--log")
        .arg(&unopenable)
        .output()
        .unwrap();
    assert_eq!(bad_log.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad_log.stderr).contains("missing/requests.ndjson"));
    assert!(!socket.exists(), "it stops before listening");
}

fn script_value() -> Value {
    json!({"methods": {"v1.system.status": [{"result": {"version": "mock"}}]}})
}

struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
