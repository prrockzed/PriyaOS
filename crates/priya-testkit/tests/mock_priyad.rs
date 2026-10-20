//! mock-priyad answers from its script, as priyad would on the wire.

use std::time::Duration;

use priya_api::client::{Client, ClientError};
use priya_testkit::mock_priyad::{Script, serve};
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
                {"delay_ms": 0, "method": "v1.events.event", "params": {"subscription": "sub-1", "event": {"type": "setting.changed"}}}
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
