//! `model_provider_contract` against recorded answers from real servers
//! (Ollama and Gemini), replayed through a scripted local server, so the
//! adapter is checked against what those servers really send.
//!
//! Record again with, for Ollama (needs `ollama serve` and
//! `ollama pull qwen2.5:0.5b`):
//!
//! ```text
//! PRIYA_RECORD=ollama cargo test -p priya-ai --test cassettes -- --exact record
//! ```
//!
//! and for Gemini, on Linux after `priya ai key set gemini` (the key is
//! read from the keyring and never written down):
//!
//! ```text
//! PRIYA_RECORD=gemini cargo test -p priya-ai --test cassettes -- --exact record
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use priya_ai::openai_compatible::{Endpoint, OpenAiCompatible, http_client, wire};
use priya_ai::{Message, ModelProvider, ModelRequest, ReasoningEffort, Reliable};
use priya_testkit::cassette::Cassette;
use priya_testkit::http_script::ScriptedHttp;
use priya_testkit::model_contract::{self as contract, Scenario, Setup, TIMEOUTS};

/// What every recording asks: short, so free tiers and small models
/// answer quickly and cheaply. Gemini counts its thinking against the
/// limit, so it gets more room, and thinks least, as PriyaOS's default
/// settings ask (`ai.providers.gemini.reasoning_effort`).
fn question(model: &str) -> ModelRequest {
    let gemini = model.starts_with("gemini");
    ModelRequest::new(model, vec![Message::user("Say hello in five words.")])
        .with_max_output_tokens(if gemini { 512 } else { 16 })
        .with_reasoning_effort(gemini.then_some(ReasoningEffort::Minimal))
}

fn folder(provider: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/cassettes/{provider}"))
}

/// A recorded answer as a contract subject. Only `Scenario::Answer` was
/// recorded, so only the checks that use it run here.
struct CassetteSetup {
    cassette: Cassette,
    server: ScriptedHttp,
}

impl CassetteSetup {
    async fn load(provider: &str, scenario: Scenario) -> Self {
        assert_eq!(scenario, Scenario::Answer, "only answers are recorded");
        let cassette = Cassette::load(&folder(provider), "answer");
        let server = cassette.serve().await.expect("a local port");
        Self { cassette, server }
    }
}

impl Setup for CassetteSetup {
    fn provider(&self) -> Arc<dyn ModelProvider> {
        let endpoint = Endpoint {
            id: "recorded".into(),
            label: "The recorded server".into(),
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

    fn request(&self) -> ModelRequest {
        let model = self.cassette.request.body["model"]
            .as_str()
            .unwrap_or_default();
        question(model)
    }

    fn expected_answer(&self) -> String {
        self.cassette.text.clone()
    }
}

/// The adapter still asks exactly what was recorded, so the recording
/// stays a fair stand-in for the real server.
async fn asks_what_was_recorded(provider: &str) {
    let setup = CassetteSetup::load(provider, Scenario::Answer).await;
    setup
        .provider()
        .complete(setup.request())
        .await
        .expect("the recording answers");
    let sent: serde_json::Value =
        serde_json::from_slice(&setup.server.requests()[0].body).expect("a JSON body");
    assert_eq!(sent, setup.cassette.request.body);
    let path = format!("/v1{}", setup.cassette.request.path);
    assert_eq!(setup.server.requests()[0].path, path);
}

/// The recording cut at awkward places (inside lines, inside characters)
/// still gives the same answer.
async fn reads_it_cut_anywhere(provider: &str) {
    let cassette = Cassette::load(&folder(provider), "answer");
    for every in [1, 7, 64] {
        let server = ScriptedHttp::start(vec![cassette.reply_split(every)])
            .await
            .expect("a local port");
        let setup = CassetteSetup {
            cassette: cassette.clone(),
            server,
        };
        let events: Vec<_> = tokio_stream::StreamExt::collect(
            setup
                .provider()
                .complete(setup.request())
                .await
                .expect("it answers"),
        )
        .await;
        let text: String = events
            .iter()
            .filter_map(|e| match e {
                Ok(priya_ai::StreamEvent::TextDelta(t)) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, cassette.text, "cut every {every} bytes");
    }
}

mod ollama {
    use super::*;

    async fn setup(scenario: Scenario) -> CassetteSetup {
        CassetteSetup::load("ollama", scenario).await
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn streams_text_deltas_in_order() {
        contract::streams_text_deltas_in_order(setup).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn reports_usage_at_end() {
        contract::reports_usage_at_end(setup).await;
    }

    // covers: AI-21
    // suite: contract
    #[tokio::test]
    async fn asks_what_was_recorded() {
        super::asks_what_was_recorded("ollama").await;
    }

    // covers: AI-21
    // suite: contract
    #[tokio::test]
    async fn reads_it_cut_anywhere() {
        super::reads_it_cut_anywhere("ollama").await;
    }
}

// covers: AI-21
// suite: live
#[tokio::test]
async fn record() {
    let Ok(provider) = std::env::var("PRIYA_RECORD") else {
        eprintln!("skipped: set PRIYA_RECORD=ollama or gemini to record");
        return;
    };
    let (base_url, model, key) = match provider.as_str() {
        "ollama" => ("http://127.0.0.1:11434/v1", "qwen2.5:0.5b", None),
        "gemini" => {
            let key =
                priya_ai::keys::SecretStore::get(&priya_ai::keys::SecretServiceStore, "gemini")
                    .await
                    .expect("the keyring works")
                    .expect("run `priya ai key set gemini` first");
            (
                "https://generativelanguage.googleapis.com/v1beta/openai",
                priya_ai::gateway::route::GEMINI_MODEL,
                Some(key),
            )
        }
        other => panic!("can't record {other}; use ollama or gemini"),
    };
    let body = serde_json::to_value(wire::request_body(&question(model))).expect("JSON");
    let key = key.as_ref().map(|k| k.expose().as_str());
    let cassette = Cassette::record(base_url, "/chat/completions", body, key)
        .await
        .expect("the server answers");
    assert_eq!(cassette.response.status, 200, "{:?}", cassette.response);
    assert!(!cassette.text.is_empty());
    cassette
        .save(&folder(&provider), "answer")
        .expect("written");
    eprintln!("recorded: {}", cassette.text);
}
