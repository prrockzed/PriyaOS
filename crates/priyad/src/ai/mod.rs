//! Priya AI inside priyad: answering questions (0.12).

pub mod answer;
pub mod intents;
pub mod record;
pub mod running;

use std::path::Path;
use std::sync::Arc;

use priya_ai::Timeouts;
use priya_ai::gateway::Connector;
use priya_ai::keys::SecretStore;
use priya_core::clock::Clock;
use priya_store::EventStore;
use time::UtcOffset;
use tracing::warn;

use priya_api::error::{ApiError, ErrorKind};
use priya_api::methods::ai::{AskParams, AskResult};
use priya_core::settings::Ai;
use tokio::sync::{mpsc, oneshot};

use self::answer::{Answerer, Outcome, Question};
use self::running::{FreeOnDrop, Running, StopOnDrop};

/// Where the answer prompt is installed.
pub const PROMPT_FILE: &str = "/usr/share/priya/prompts/answer.md";

/// The answer prompt from `path`, or the copy built into priyad when it
/// can't be read (a development run, or a broken install).
pub fn prompt(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        warn!(path = %path.display(), error = %e, "using the built-in answer prompt");
        priya_ai::gateway::prompt::BUILT_IN.to_owned()
    })
}

/// The answerer priyad uses: real providers, keys from `keys`.
pub fn answerer(
    keys: Arc<dyn SecretStore>,
    store: EventStore,
    clock: Arc<dyn Clock>,
    prompt: String,
    offset: UtcOffset,
) -> Result<Answerer, priya_ai::ProviderError> {
    let connector = Connector::new(keys, Timeouts::default())?;
    Ok(Answerer::new(
        Arc::new(connector),
        store,
        clock,
        prompt,
        offset,
    ))
}

/// Asks and waits for the whole answer (`v1.ai.ask`, `host.ai.ask`): a
/// new conversation each time, recorded like any other question. It runs
/// among the answers being written, so it counts toward their limit and
/// priyad stopping stops it; if the asker goes away (this future is
/// dropped: a module stops, a connection is stopped), the answer stops too.
pub async fn ask_whole(
    answerer: &Arc<Answerer>,
    running: &Arc<Running>,
    ai: Ai,
    params: AskParams,
    surface: String,
) -> Result<AskResult, ApiError> {
    if params.prompt.trim().is_empty() {
        return Err(ApiError::new(
            ErrorKind::InvalidInput,
            "Ask something first.",
        ));
    }
    check_role(params.role.as_deref())?;
    let id = ulid::Ulid::generate();
    let intent_id = id.to_string();
    let question = Question {
        id,
        text: params.prompt,
        conversation: ulid::Ulid::generate().to_string(),
        surface,
        model: None,
        role: params.role,
    };
    let stopped = running.add(&intent_id)?;
    let (done, outcome) = oneshot::channel();
    running.spawn({
        let answerer = answerer.clone();
        let place = FreeOnDrop {
            running: running.clone(),
            intent_id: intent_id.clone(),
        };
        async move {
            let _place = place;
            // Nobody reads the steps: the answer comes back whole.
            let (updates, _) = mpsc::channel(1);
            let _ = done.send(answerer.answer(&ai, question, updates, stopped).await);
        }
    });
    let _asker = StopOnDrop {
        running: running.clone(),
        intent_id,
    };
    match outcome.await {
        Ok(Outcome::Answered { text, model }) => Ok(AskResult {
            answer: text,
            model,
        }),
        Ok(Outcome::Failed(error)) => Err(error),
        Ok(Outcome::Cancelled { .. }) => Err(ApiError::new(
            ErrorKind::Cancelled,
            "The answer was stopped.",
        )),
        Err(_) => Err(ApiError::new(
            ErrorKind::Internal,
            "Something went wrong while answering.",
        )),
    }
}

/// Roles name a kind of job (`answer`, `summarize`); routing by role comes
/// in 1.7. `private` means "never leave this computer", which Phase 0
/// can't promise yet, so it is refused rather than sent anywhere.
fn check_role(role: Option<&str>) -> Result<(), ApiError> {
    let Some(role) = role else {
        return Ok(());
    };
    let word = (1..=64).contains(&role.len())
        && role
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'));
    if !word {
        return Err(ApiError::new(
            ErrorKind::InvalidInput,
            "A role is one word, such as answer or summarize.",
        ));
    }
    if role == "private" {
        return Err(ApiError::new(
            ErrorKind::InvalidInput,
            "The private role keeps a question on this computer; it arrives with local-only routing in Phase 1.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use priya_ai::gateway::{ConnectError, Route};
    use priya_ai::keys::KeyError;
    use priya_ai::provider::BoxFuture;
    use priya_ai::{ModelProvider, StreamEvent};
    use priya_core::settings::{Provider, ProviderKind, ProviderName};
    use priya_store::StoreConfig;
    use priya_testkit::{FakeClock, FakeModelProvider, Reply, Step};
    use time::macros::datetime;

    use super::answer::ProviderSource;
    use super::*;

    struct Fake(Arc<FakeModelProvider>);

    impl ProviderSource for Fake {
        fn provider<'a>(
            &'a self,
            _: &'a Route,
        ) -> BoxFuture<'a, Result<Arc<dyn ModelProvider>, ConnectError>> {
            let provider: Arc<dyn ModelProvider> = self.0.clone();
            Box::pin(async move { Ok(provider) })
        }

        fn key_stored<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<bool, KeyError>> {
            Box::pin(async { Ok(false) })
        }
    }

    fn ai() -> Ai {
        let mut providers = BTreeMap::new();
        providers.insert(
            ProviderName::try_from("local".to_owned()).unwrap(),
            Provider {
                kind: ProviderKind::OpenaiCompatible,
                base_url: Some("http://127.0.0.1:11434/v1".into()),
                enabled: true,
            },
        );
        Ai {
            strategy: Some("fixed:local/m".parse().unwrap()),
            providers,
            max_output_tokens: 64,
        }
    }

    fn ask(prompt: &str, role: Option<&str>) -> AskParams {
        AskParams {
            prompt: prompt.into(),
            role: role.map(str::to_owned),
        }
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn a_whole_answer_stops_when_its_asker_goes_away() {
        let dir = tempfile::tempdir().unwrap();
        let clock = Arc::new(FakeClock::starting_at(datetime!(2026-10-21 08:00 UTC)));
        let store = EventStore::open(&StoreConfig::in_data_dir(dir.path()), clock.clone()).unwrap();
        let fake = FakeModelProvider::new("local");
        fake.push(Reply::Stream(vec![
            Step::Event(StreamEvent::Started { model: "m".into() }),
            Step::Hang,
        ]));
        let fake = Arc::new(fake);
        let answerer = Arc::new(Answerer::new(
            Arc::new(Fake(fake.clone())),
            store.clone(),
            clock,
            String::new(),
            UtcOffset::UTC,
        ));
        let running = Arc::new(Running::default());
        let asking = ask_whole(
            &answerer,
            &running,
            ai(),
            ask("hi", None),
            "module:echo".into(),
        );
        // The asker gives up (a module stopped) while the model is answering.
        let gave_up = tokio::time::timeout(Duration::from_millis(200), asking).await;
        assert!(gave_up.is_err(), "it was still answering");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let events = store.read_after(0, 10).await.unwrap();
            if let Some(cancelled) = events
                .iter()
                .find(|r| r.event.event_type == "intent.cancelled")
            {
                assert_eq!(
                    cancelled.event.payload["reason"],
                    "the asking connection closed"
                );
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the answer didn't stop"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        running.shutdown(Duration::from_secs(5)).await;
        assert!(
            running.add("after").is_err(),
            "its place was freed, and priyad is stopping"
        );
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn roles_are_words_and_private_waits_for_local_routing() {
        assert!(check_role(None).is_ok());
        assert!(check_role(Some("summarize")).is_ok());
        for bad in ["", "two words", &"x".repeat(65), "ai.role"] {
            let refused = check_role(Some(bad)).unwrap_err();
            assert_eq!(refused.kind, ErrorKind::InvalidInput, "{bad:?}");
        }
        let private = check_role(Some("private")).unwrap_err();
        assert!(private.user_message.contains("on this computer"));
    }
}
