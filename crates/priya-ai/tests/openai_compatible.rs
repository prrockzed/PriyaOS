//! The `openai_compatible` adapter over real HTTP, against a scripted
//! local server: what it sends, and how it reads what comes back.

use std::sync::Arc;
use std::time::Duration;

use priya_ai::openai_compatible::{Endpoint, OpenAiCompatible, http_client};
use priya_ai::{
    Health, Message, ModelProvider, ModelRequest, ProviderError, ProviderErrorKind, StreamEvent,
};
use priya_core::secret::Secret;
use priya_testkit::http_script::{BodyStep, HttpReply, ScriptedHttp};
use serde_json::Value;
use tokio_stream::StreamExt;

const KEY: &str = "test-key-not-real";

fn adapter(base_url: String, key: bool) -> OpenAiCompatible {
    let endpoint = Endpoint {
        id: "gemini".into(),
        label: "Gemini".into(),
        base_url,
        key: key.then(|| Arc::new(Secret::new(KEY.to_owned()))),
    };
    OpenAiCompatible::new(endpoint, http_client().expect("a client"))
}

fn ask() -> ModelRequest {
    ModelRequest::new("m-1", vec![Message::user("hi")]).with_max_output_tokens(16)
}

async fn answer(adapter: &OpenAiCompatible) -> Vec<Result<StreamEvent, ProviderError>> {
    adapter
        .complete(ask())
        .await
        .expect("it starts")
        .collect()
        .await
}

const HELLO: [&str; 4] = [
    r#"{"model":"m-1","choices":[{"delta":{"content":"Good"},"finish_reason":null}]}"#,
    r#"{"model":"m-1","choices":[{"delta":{"content":" day"},"finish_reason":"stop"}]}"#,
    r#"{"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":2}}"#,
    "[DONE]",
];

// covers: AI-21
// suite: integration
#[tokio::test]
async fn sends_the_key_as_a_bearer_token_and_asks_for_a_stream_with_usage() {
    let server = ScriptedHttp::start(vec![HttpReply::sse(&HELLO)])
        .await
        .unwrap();
    let events = answer(&adapter(server.base_url(), true)).await;
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            Ok(StreamEvent::TextDelta(t)) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Good day");
    assert!(events.iter().all(Result::is_ok), "{events:?}");

    let request = &server.requests()[0];
    assert_eq!(
        (request.method.as_str(), request.path.as_str()),
        ("POST", "/v1/chat/completions")
    );
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["model"], "m-1");
}

// covers: AI-21
// suite: integration
#[tokio::test]
async fn a_local_server_gets_no_authorization_header() {
    let server = ScriptedHttp::start(vec![HttpReply::sse(&HELLO)])
        .await
        .unwrap();
    answer(&adapter(server.base_url(), false)).await;
    assert_eq!(server.requests()[0].header("authorization"), None);
}

// covers: AI-21
// suite: integration
#[tokio::test]
async fn refusals_keep_retry_after_and_never_echo_the_key() {
    let server = ScriptedHttp::start(vec![
        HttpReply::status(429, "{}").header("retry-after", "7"),
        HttpReply::status(400, &format!(r#"{{"error":"API key not valid: {KEY}"}}"#)),
    ])
    .await
    .unwrap();
    let adapter = adapter(server.base_url(), true);
    let limited = adapter.complete(ask()).await.unwrap_err();
    assert_eq!(limited.kind, ProviderErrorKind::RateLimited);
    assert_eq!(limited.retry_after, Some(Duration::from_secs(7)));

    let refused = adapter.complete(ask()).await.unwrap_err();
    assert_eq!(refused.kind, ProviderErrorKind::Auth);
    let detail = refused.detail.unwrap();
    assert!(
        detail.contains("HTTP 400") && detail.contains("[key]"),
        "{detail}"
    );
    assert!(!detail.contains(KEY), "{detail}");
}

// covers: AI-21
// suite: integration
#[tokio::test]
async fn bad_lines_and_error_lines_end_the_stream_with_a_plain_error() {
    let server = ScriptedHttp::start(vec![
        HttpReply::sse(&[HELLO[0], "{not json"]),
        HttpReply::sse_steps(vec![
            BodyStep::Send(format!("data: {}\n\n", HELLO[0]).into_bytes()),
            BodyStep::Send(br#"error: {"code":500,"message":"out of memory"}"#.to_vec()),
            BodyStep::Send(b"\n\n".to_vec()),
        ]),
    ])
    .await
    .unwrap();
    let adapter = adapter(server.base_url(), false);
    let unreadable = answer(&adapter).await.pop().unwrap().unwrap_err();
    assert_eq!(
        unreadable.user_message,
        "Gemini sent an answer PriyaOS couldn't read."
    );
    let failed = answer(&adapter).await.pop().unwrap().unwrap_err();
    assert_eq!(failed.user_message, "Gemini had a problem while answering.");
    assert!(failed.detail.unwrap().contains("out of memory"));
}

// covers: AI-21
// suite: integration
#[tokio::test]
async fn dropping_the_stream_closes_the_connection() {
    let server = ScriptedHttp::start(vec![HttpReply::sse_steps(vec![
        BodyStep::Send(format!("data: {}\n\n", HELLO[0]).into_bytes()),
        BodyStep::Hang,
    ])])
    .await
    .unwrap();
    let adapter = adapter(server.base_url(), false);
    let mut stream = adapter.complete(ask()).await.unwrap();
    while !matches!(stream.next().await, Some(Ok(StreamEvent::TextDelta(_)))) {}
    assert_eq!(server.open(), 1);
    drop(stream);
    let deadline = tokio::time::Instant::now() + Duration::from_millis(100);
    while server.open() > 0 {
        assert!(tokio::time::Instant::now() < deadline, "still open");
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

// covers: AI-21
// suite: integration
#[tokio::test]
async fn health_asks_for_the_model_list_and_explains_failures() {
    let server = ScriptedHttp::start(vec![
        HttpReply::status(200, r#"{"data":[]}"#),
        HttpReply::status(401, "{}"),
        HttpReply::status(404, "{}"),
    ])
    .await
    .unwrap();
    let adapter = adapter(server.base_url(), true);
    assert_eq!(adapter.health().await, Health::Ok);
    let Health::Down(refused) = adapter.health().await else {
        panic!("a refused key is down")
    };
    assert_eq!(refused.kind, ProviderErrorKind::Auth);
    let Health::Down(moved) = adapter.health().await else {
        panic!("a wrong address is down")
    };
    assert!(
        moved
            .user_message
            .starts_with("Gemini isn't at http://127.0.0.1:"),
        "{}",
        moved.user_message
    );
    let request = &server.requests()[0];
    assert_eq!(
        (request.method.as_str(), request.path.as_str()),
        ("GET", "/v1/models")
    );
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
}

// covers: AI-21
// suite: integration
#[tokio::test(start_paused = true)]
async fn health_gives_up_after_two_seconds_and_a_closed_port_is_unreachable() {
    // Accepts connections and never answers.
    let silent = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = silent.local_addr().unwrap();
    let hold = tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = silent.accept().await {
            held.push(socket);
        }
    });
    let begun = tokio::time::Instant::now();
    let Health::Down(slow) = adapter(format!("http://{address}/v1"), false)
        .health()
        .await
    else {
        panic!("a silent server is down")
    };
    assert_eq!(slow.kind, ProviderErrorKind::Network);
    assert!(begun.elapsed() >= Duration::from_secs(2));
    hold.abort();

    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let error = adapter(format!("http://127.0.0.1:{port}/v1"), false)
        .complete(ask())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::Network);
    assert_eq!(error.user_message, "Couldn't reach Gemini.");
    assert!(error.detail.unwrap().to_lowercase().contains("refused"));
}
