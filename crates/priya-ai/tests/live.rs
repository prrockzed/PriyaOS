//! Live smoke (0.12): one real question to Gemini's free tier and one to
//! the local Ollama, each with `max_output_tokens = 16`. Outside CI and
//! skipped unless `PRIYA_LIVE=1`:
//!
//! ```text
//! PRIYA_LIVE=1 cargo xtask test live
//! ```
//!
//! Gemini needs `priya ai key set gemini` first (Linux; the key is read
//! from the keyring). `PRIYA_LIVE_LOCAL_MODEL` picks the Ollama model
//! (`qwen2.5:0.5b` by default). Only a test question is sent: free tiers
//! may learn from what they are asked.

// A smoke test reports what the models said.
#![allow(clippy::print_stderr)]

use std::sync::Arc;

use priya_ai::keys::{SecretServiceStore, SecretStore};
use priya_ai::openai_compatible::{Endpoint, OpenAiCompatible, http_client};
use priya_ai::{
    Message, ModelProvider, ModelRequest, ReasoningEffort, Reliable, StreamEvent, Timeouts,
};
use tokio_stream::StreamExt;

fn live() -> bool {
    // Only "1": PRIYA_LIVE=0 mustn't spend anything.
    let on = std::env::var("PRIYA_LIVE").is_ok_and(|v| v == "1");
    if !on {
        eprintln!("skipped: set PRIYA_LIVE=1 to call real models");
    }
    on
}

/// Asks the smoke question and returns the answer's text; any error in
/// the stream fails the test.
async fn smoke(endpoint: Endpoint, model: &str, effort: Option<ReasoningEffort>) -> String {
    let adapter = OpenAiCompatible::new(endpoint, http_client().expect("a client"));
    let provider = Reliable::new(Arc::new(adapter), Timeouts::default());
    assert_eq!(
        provider.health().await,
        priya_ai::Health::Ok,
        "the provider answers"
    );
    let request = ModelRequest::new(model, vec![Message::user("Reply with one word: hello.")])
        .with_max_output_tokens(16)
        .with_reasoning_effort(effort);
    let events: Vec<_> = provider
        .complete(request)
        .await
        .expect("the question is taken")
        .collect()
        .await;
    let mut text = String::new();
    for event in events {
        match event.expect("no error while answering") {
            StreamEvent::TextDelta(delta) => text.push_str(&delta),
            StreamEvent::Usage(usage) => eprintln!("usage: {usage:?}"),
            other => eprintln!("{other:?}"),
        }
    }
    eprintln!("answer: {text:?}");
    text
}

// covers: AI-21
// suite: live
#[tokio::test]
async fn live_ollama_answers() {
    if !live() {
        return;
    }
    let model = std::env::var("PRIYA_LIVE_LOCAL_MODEL").unwrap_or_else(|_| "qwen2.5:0.5b".into());
    let endpoint = Endpoint {
        id: "local".into(),
        label: "the local model server".into(),
        base_url: "http://127.0.0.1:11434/v1".into(),
        key: None,
    };
    assert!(!smoke(endpoint, &model, None).await.is_empty());
}

// covers: AI-21
// suite: live
#[tokio::test]
async fn live_gemini_answers() {
    if !live() {
        return;
    }
    let key = SecretServiceStore
        .get("gemini")
        .await
        .expect("the keyring works")
        .expect("run `priya ai key set gemini` first");
    let endpoint = Endpoint {
        id: "gemini".into(),
        label: "Gemini".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
        key: Some(Arc::new(key)),
    };
    // As PriyaOS asks by default: the suggested model, thinking least.
    // It may still spend all 16 tokens thinking and say nothing; the call
    // must end cleanly either way.
    smoke(
        endpoint,
        priya_ai::gateway::route::GEMINI_MODEL,
        Some(ReasoningEffort::Minimal),
    )
    .await;
}
