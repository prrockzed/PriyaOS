//! `model_provider_contract` against the `openai_compatible` adapter, over
//! real HTTP to a scripted local server that answers the way Chat
//! Completions servers do.

use std::sync::Arc;
use std::time::Duration;

use priya_ai::openai_compatible::{Endpoint, OpenAiCompatible, http_client};
use priya_ai::{ModelProvider, Reliable};
use priya_testkit::http_script::{BodyStep, HttpReply, ScriptedHttp};
use priya_testkit::model_contract::{self as contract, CHUNKS, Scenario, Setup, TIMEOUTS};

/// A Chat Completions chunk carrying `text`.
fn chunk(text: &str) -> String {
    format!(
        r#"{{"model":"contract-model","choices":[{{"delta":{{"content":"{text}"}},"finish_reason":null}}]}}"#
    )
}

fn event(data: &str) -> BodyStep {
    BodyStep::Send(format!("data: {data}\n\n").into_bytes())
}

fn answer() -> HttpReply {
    let mut events: Vec<String> = CHUNKS.iter().map(|c| chunk(c)).collect();
    events.push(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.into());
    events.push(r#"{"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":3}}"#.into());
    events.push("[DONE]".into());
    HttpReply::sse(&events.iter().map(String::as_str).collect::<Vec<_>>())
}

fn busy() -> HttpReply {
    HttpReply::status(503, r#"{"error":{"message":"The model is overloaded."}}"#)
}

struct OpenAiSetup {
    server: ScriptedHttp,
}

impl OpenAiSetup {
    async fn new(scenario: Scenario) -> Self {
        let replies = match scenario {
            Scenario::Answer => vec![answer()],
            Scenario::Unauthorized => vec![HttpReply::status(401, r#"{"error":"bad key"}"#)],
            Scenario::RateLimited => vec![
                HttpReply::status(429, "{}")
                    .header("retry-after", &contract::RETRY_AFTER.as_secs().to_string()),
            ],
            Scenario::OverloadedOnce => vec![busy(), answer()],
            Scenario::OverloadedAlways => vec![busy(), busy(), busy()],
            Scenario::FailsAfterFirstToken => vec![HttpReply::sse(&[
                &chunk("Hello"),
                r#"{"error":{"message":"The model is overloaded."}}"#,
            ])],
            Scenario::StallsAfterFirstToken => {
                vec![HttpReply::sse_steps(vec![
                    event(&chunk("Hello")),
                    BodyStep::Hang,
                ])]
            }
            Scenario::Slow => {
                let mut steps = Vec::new();
                for _ in 0..100 {
                    steps.push(event(&chunk(".")));
                    steps.push(BodyStep::Pause(Duration::from_millis(100)));
                }
                vec![HttpReply::sse_steps(steps)]
            }
            Scenario::Malformed => vec![HttpReply::sse(&[&chunk("Hello"), "{not json"])],
        };
        Self {
            server: ScriptedHttp::start(replies).await.expect("a local port"),
        }
    }
}

impl Setup for OpenAiSetup {
    fn provider(&self) -> Arc<dyn ModelProvider> {
        let endpoint = Endpoint {
            id: "scripted".into(),
            label: "The scripted server".into(),
            base_url: self.server.base_url(),
            key: None,
        };
        let adapter = OpenAiCompatible::new(endpoint, http_client().expect("a client"));
        Arc::new(Reliable::new(Arc::new(adapter), TIMEOUTS))
    }

    fn requests(&self) -> usize {
        self.server.requests().len()
    }

    fn open(&self) -> usize {
        self.server.open()
    }
}

mod openai_compatible {
    use super::*;

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn streams_text_deltas_in_order() {
        contract::streams_text_deltas_in_order(OpenAiSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn reports_usage_at_end() {
        contract::reports_usage_at_end(OpenAiSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn maps_401_to_auth_error_with_plain_message() {
        contract::maps_401_to_auth_error_with_plain_message(OpenAiSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn maps_429_with_retry_after() {
        contract::maps_429_with_retry_after(OpenAiSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn retries_overloaded_once_before_first_token() {
        contract::retries_overloaded_once_before_first_token(OpenAiSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn never_retries_after_first_token() {
        contract::never_retries_after_first_token(OpenAiSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn idle_timeout_yields_timeout_error() {
        contract::idle_timeout_yields_timeout_error(OpenAiSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn cancel_drops_connection_within_100ms() {
        contract::cancel_drops_connection_within_100ms(OpenAiSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn malformed_sse_line_is_error_not_panic() {
        contract::malformed_sse_line_is_error_not_panic(OpenAiSetup::new).await;
    }
}
