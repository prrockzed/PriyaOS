//! Answering one question: the route, the provider and the streamed
//! answer, recorded as `record` describes.

use std::sync::Arc;

use priya_ai::gateway::prompt::MAX_TURNS;
use priya_ai::gateway::{ConnectError, Connector, Route, Turn, conversation, route, system_prompt};
use priya_ai::keys::KeyError;
use priya_ai::provider::BoxFuture;
use priya_ai::{ModelProvider, ModelRequest, ProviderError};
use priya_api::error::{ApiError, ErrorKind};
use priya_api::events::CallOutcome;
use priya_core::clock::Clock;
use priya_core::settings::Ai;
use priya_store::EventStore;
use time::UtcOffset;
use tokio::sync::{mpsc, watch};
use tokio_stream::StreamExt;
use tracing::warn;

use super::record::{Call, IntentLog};
pub use super::record::{Outcome, Question, Stop, Update};

/// Where providers come from: the [`Connector`] for real, a fake in tests.
pub trait ProviderSource: Send + Sync {
    fn provider<'a>(
        &'a self,
        route: &'a Route,
    ) -> BoxFuture<'a, Result<Arc<dyn ModelProvider>, ConnectError>>;

    /// Whether a key is stored for `provider` (an error: no keyring).
    fn key_stored<'a>(&'a self, provider: &'a str) -> BoxFuture<'a, Result<bool, KeyError>>;
}

impl ProviderSource for Connector {
    fn provider<'a>(
        &'a self,
        route: &'a Route,
    ) -> BoxFuture<'a, Result<Arc<dyn ModelProvider>, ConnectError>> {
        Box::pin(Connector::provider(self, route))
    }

    fn key_stored<'a>(&'a self, provider: &'a str) -> BoxFuture<'a, Result<bool, KeyError>> {
        Box::pin(Connector::key_stored(self, provider))
    }
}

pub struct Answerer {
    providers: Arc<dyn ProviderSource>,
    store: EventStore,
    clock: Arc<dyn Clock>,
    /// The system prompt with `{date}` still in it.
    prompt: String,
    /// The owner's time zone, for today's date in the prompt; read once at
    /// start-up, as Linux can't tell a multi-threaded program later (so
    /// after a clock change the date can be off for an hour at midnight).
    offset: UtcOffset,
}

impl Answerer {
    pub fn new(
        providers: Arc<dyn ProviderSource>,
        store: EventStore,
        clock: Arc<dyn Clock>,
        prompt: String,
        offset: UtcOffset,
    ) -> Self {
        Self {
            providers,
            store,
            clock,
            prompt,
            offset,
        }
    }

    /// Where providers come from, for status and doctor.
    pub fn providers(&self) -> &dyn ProviderSource {
        self.providers.as_ref()
    }

    /// Answers `question` with the settings `ai`, sending updates as they
    /// come, until done or until `cancel` turns true.
    pub async fn answer(
        &self,
        ai: &Ai,
        question: Question,
        updates: mpsc::Sender<Update>,
        mut cancel: watch::Receiver<Option<Stop>>,
    ) -> Outcome {
        let log = IntentLog::new(&self.store, &question);
        log.submitted(&question).await;
        let route = match route(ai, question.model.as_deref()) {
            Ok(route) => route,
            Err(e) => return log.failed(ApiError::from_user_facing(&e)).await,
        };
        // Reading the key can wait on the keyring (an unlock prompt, a
        // slow D-Bus); a stop mustn't wait with it.
        let prepared = tokio::select! {
            biased;
            stop = stopped(&mut cancel) => return log.cancelled(stop, String::new()).await,
            prepared = self.prepare(ai, &question, &route) => prepared,
        };
        let (provider, request) = match prepared {
            Ok(prepared) => prepared,
            Err(e) => return log.failed(ApiError::from_user_facing(&e)).await,
        };
        let clock = &*self.clock;
        let mut call = Call::start(clock, &route, question.role.as_deref());
        let started = tokio::select! {
            biased;
            stop = stopped(&mut cancel) => return cancel_call(&log, call, clock, stop).await,
            started = provider.complete(request) => started,
        };
        let mut stream = match started {
            Ok(stream) => stream,
            Err(e) => return fail_call(&log, &call, clock, &e).await,
        };
        loop {
            // Dropping the stream on a stop closes the connection.
            let event = tokio::select! {
                biased;
                stop = stopped(&mut cancel) => return cancel_call(&log, call, clock, stop).await,
                event = stream.next() => event,
            };
            let update = match event {
                None => break,
                Some(Err(e)) => return fail_call(&log, &call, clock, &e).await,
                Some(Ok(event)) => call.take(clock, event, &route),
            };
            if let Some(update) = update {
                // An asker that has gone stops reading; the answer is
                // still finished and recorded. One that is slow to read
                // can still be stopped.
                tokio::select! {
                    biased;
                    stop = stopped(&mut cancel) => return cancel_call(&log, call, clock, stop).await,
                    _ = updates.send(update) => {}
                }
            }
        }
        log.called(call.finish(clock, CallOutcome::Ok, None)).await;
        if call.text.trim().is_empty() {
            // Nothing to show (a filtered or cut-off answer): say so
            // rather than show a blank answer.
            return log
                .failed(ApiError::new(
                    ErrorKind::ProviderError,
                    "The model gave an empty answer. Try asking again.",
                ))
                .await;
        }
        log.answered(call.text, call.model).await
    }

    /// The provider and the request: the steps before the model is asked.
    async fn prepare(
        &self,
        ai: &Ai,
        question: &Question,
        route: &Route,
    ) -> Result<(Arc<dyn ModelProvider>, ModelRequest), ConnectError> {
        let provider = self.providers.provider(route).await?;
        Ok((provider, self.request(ai, question, route).await))
    }

    async fn request(&self, ai: &Ai, question: &Question, route: &Route) -> ModelRequest {
        let today = self.clock.now().to_offset(self.offset).date();
        let system = system_prompt(&self.prompt, today);
        let earlier = self
            .store
            .conversation_turns(&question.conversation, MAX_TURNS)
            .await
            .unwrap_or_else(|e| {
                warn!(error = %e, "can't read the conversation; answering without it");
                Vec::new()
            })
            .into_iter()
            .map(|(question, answer)| Turn { question, answer })
            .collect::<Vec<_>>();
        let messages = conversation(&system, &earlier, &question.text);
        ModelRequest::new(route.model.clone(), messages)
            .with_system(system)
            .with_max_output_tokens(ai.max_output_tokens)
    }
}

async fn cancel_call(log: &IntentLog<'_>, call: Call, clock: &dyn Clock, stop: Stop) -> Outcome {
    log.called(call.finish(clock, CallOutcome::Cancelled, None))
        .await;
    log.cancelled(stop, call.text).await
}

async fn fail_call(
    log: &IntentLog<'_>,
    call: &Call,
    clock: &dyn Clock,
    error: &ProviderError,
) -> Outcome {
    let error = ApiError::from_user_facing(error);
    log.called(call.finish(clock, CallOutcome::Error, Some(&error)))
        .await;
    log.failed(error).await
}

/// Resolves with the reason once `cancel` says stop. A sender that is gone
/// never stops it.
async fn stopped(cancel: &mut watch::Receiver<Option<Stop>>) -> Stop {
    let stop = match cancel.wait_for(Option::is_some).await {
        Ok(stop) => *stop,
        Err(_) => None,
    };
    match stop {
        Some(stop) => stop,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use priya_ai::{ProviderError, ProviderErrorKind, StreamEvent};
    use priya_api::error::ErrorKind;
    use priya_core::settings::{Provider, ProviderKind, ProviderName};
    use priya_store::StoreConfig;
    use priya_testkit::{FakeClock, FakeModelProvider, Reply, Step};
    use time::macros::datetime;
    use ulid::Ulid;

    use super::*;

    /// How the fake connector answers.
    #[derive(Clone, Copy)]
    enum Connect {
        Ok,
        NeedsKey,
        /// Waits forever, like a keyring waiting to be unlocked.
        Hang,
    }

    struct Fake(Arc<FakeModelProvider>, Connect);

    impl ProviderSource for Fake {
        fn provider<'a>(
            &'a self,
            _: &'a Route,
        ) -> BoxFuture<'a, Result<Arc<dyn ModelProvider>, ConnectError>> {
            let provider: Arc<dyn ModelProvider> = self.0.clone();
            let connect = self.1;
            Box::pin(async move {
                match connect {
                    Connect::Ok => Ok(provider),
                    Connect::NeedsKey => Err(ConnectError::NeedsKey {
                        provider: "gemini".into(),
                        label: "Gemini".into(),
                    }),
                    Connect::Hang => std::future::pending().await,
                }
            })
        }

        fn key_stored<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<bool, KeyError>> {
            Box::pin(async { Ok(false) })
        }
    }

    struct Setup {
        _dir: tempfile::TempDir,
        store: EventStore,
        fake: Arc<FakeModelProvider>,
        answerer: Answerer,
    }

    fn setup(replies: Vec<Reply>) -> Setup {
        setup_with(replies, Connect::Ok)
    }

    fn setup_with(replies: Vec<Reply>, connect: Connect) -> Setup {
        let dir = tempfile::tempdir().unwrap();
        // 23:30 UTC is already the next day in India.
        let clock = Arc::new(FakeClock::starting_at(datetime!(2026-10-20 23:30 UTC)));
        let store = EventStore::open(&StoreConfig::in_data_dir(dir.path()), clock.clone()).unwrap();
        let fake = FakeModelProvider::new("local");
        for reply in replies {
            fake.push(reply);
        }
        let fake = Arc::new(fake);
        let answerer = Answerer::new(
            Arc::new(Fake(fake.clone(), connect)),
            store.clone(),
            clock,
            "Today is {date}.".into(),
            UtcOffset::from_hms(5, 30, 0).unwrap(),
        );
        Setup {
            _dir: dir,
            store,
            fake,
            answerer,
        }
    }

    fn ai(strategy: Option<&str>) -> Ai {
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
            strategy: strategy.map(|s| s.parse().unwrap()),
            providers,
            max_output_tokens: 64,
        }
    }

    fn question(text: &str, conversation: &str) -> Question {
        Question {
            id: Ulid::generate(),
            text: text.into(),
            conversation: conversation.into(),
            surface: "cli".into(),
            model: None,
            role: None,
        }
    }

    async fn events(store: &EventStore) -> Vec<priya_api::events::EventEnvelope> {
        let records = store.read_after(0, 100).await.unwrap();
        records.into_iter().map(|r| r.event).collect()
    }

    /// A sender that is gone never stops the answer.
    fn never_cancelled() -> watch::Receiver<Option<Stop>> {
        watch::channel(None).1
    }

    async fn ask(
        s: &Setup,
        updates: mpsc::Sender<Update>,
        cancel: watch::Receiver<Option<Stop>>,
    ) -> Outcome {
        s.answerer
            .answer(
                &ai(Some("fixed:local/m")),
                question("hi", "c1"),
                updates,
                cancel,
            )
            .await
    }

    // covers: AI-21, CORE-01
    // suite: unit
    #[tokio::test]
    async fn an_answer_streams_and_records_its_three_events() {
        let s = setup(vec![Reply::answer(
            "qwen2.5:0.5b",
            &["Btrfs is", " a filesystem."],
        )]);
        let (tx, mut rx) = mpsc::channel(16);
        let outcome = s
            .answerer
            .answer(
                &ai(Some("fixed:local/qwen2.5:0.5b")),
                question("what is btrfs?", "c1"),
                tx,
                never_cancelled(),
            )
            .await;
        assert_eq!(
            outcome,
            Outcome::Answered {
                text: "Btrfs is a filesystem.".into(),
                model: "qwen2.5:0.5b".into()
            }
        );
        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }
        assert_eq!(
            updates,
            [
                Update::Started {
                    provider: "local".into(),
                    model: "qwen2.5:0.5b".into(),
                    local: true
                },
                Update::Delta("Btrfs is".into()),
                Update::Delta(" a filesystem.".into()),
                Update::Usage(priya_ai::Usage::new(10, 2)),
            ]
        );

        let log = events(&s.store).await;
        priya_testkit::assert_events!(
            log.iter(),
            [
                "intent.submitted" => {"text": "what is btrfs?", "conversation": "c1", "surface": "cli"},
                "model.called" => {"provider": "local", "model": "qwen2.5:0.5b", "role": "answer", "outcome": "ok", "tokens_in": 10, "tokens_out": 2},
                "intent.answered" => {"answer": "Btrfs is a filesystem."},
            ]
        );
        assert!(
            log.iter().all(|e| e.stream == log[0].stream
                && e.correlation.as_deref() == Some("conversation:c1"))
        );
        assert!(log[1..].iter().all(|e| e.causation == Some(log[0].id)));

        // The prompt has the date in India, already the 21st.
        let request = &s.fake.requests()[0];
        assert_eq!(
            request.system.as_deref(),
            Some("Today is Wednesday 21 October 2026.")
        );
        assert_eq!(request.max_output_tokens, 64);
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn a_follow_up_brings_the_earlier_turns() {
        let s = setup(vec![
            Reply::answer("m", &["Answer one."]),
            Reply::answer("m", &["Answer two."]),
        ]);
        let ai = ai(Some("fixed:local/m"));
        let (tx, _rx) = mpsc::channel(16);
        s.answerer
            .answer(&ai, question("one?", "c1"), tx.clone(), never_cancelled())
            .await;
        s.answerer
            .answer(&ai, question("two?", "c1"), tx, never_cancelled())
            .await;
        let texts: Vec<String> = s.fake.requests()[1]
            .messages
            .iter()
            .map(|m| m.text.clone())
            .collect();
        assert_eq!(texts, ["one?", "Answer one.", "two?"]);
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn without_a_model_the_question_fails_without_a_call() {
        let s = setup(Vec::new());
        let (tx, _rx) = mpsc::channel(16);
        let outcome = s
            .answerer
            .answer(&ai(None), question("hi", "c1"), tx, never_cancelled())
            .await;
        let Outcome::Failed(error) = outcome else {
            panic!("{outcome:?}")
        };
        assert_eq!(error.kind, ErrorKind::NotFound);
        assert!(error.user_message.contains("priya ai key set gemini"));
        priya_testkit::assert_events!(
            events(&s.store).await.iter(),
            ["intent.submitted", "intent.failed" => {"kind": "not_found"}]
        );
        assert!(s.fake.requests().is_empty());
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn a_refusal_is_a_failed_call_and_a_failed_intent() {
        let refused = ProviderError::new(ProviderErrorKind::Auth, "The local model needs a key.");
        let s = setup(vec![Reply::Refuse(refused)]);
        let (tx, _rx) = mpsc::channel(16);
        let outcome = s
            .answerer
            .answer(
                &ai(Some("fixed:local/m")),
                question("hi", "c1"),
                tx,
                never_cancelled(),
            )
            .await;
        assert!(
            matches!(outcome, Outcome::Failed(ref e) if e.user_message == "The local model needs a key.")
        );
        priya_testkit::assert_events!(
            events(&s.store).await.iter(),
            [
                "intent.submitted",
                "model.called" => {"outcome": "error", "error": "The local model needs a key."},
                "intent.failed" => {"kind": "provider_error", "error": "The local model needs a key."},
            ]
        );
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn cancelling_mid_stream_keeps_the_partial_answer_and_closes_the_stream() {
        let s = setup(vec![Reply::Stream(vec![
            Step::Event(StreamEvent::Started { model: "m".into() }),
            Step::Event(StreamEvent::TextDelta("Half".into())),
            Step::Hang,
        ])]);
        let (tx, mut rx) = mpsc::channel(16);
        let (stop, cancel) = watch::channel(None);
        let settings = ai(Some("fixed:local/m"));
        let answering = s
            .answerer
            .answer(&settings, question("hi", "c1"), tx, cancel);
        let stopper = async {
            while !matches!(rx.recv().await, Some(Update::Delta(_))) {}
            stop.send(Some(Stop::Asked)).unwrap();
        };
        let (outcome, ()) = tokio::join!(answering, stopper);
        assert_eq!(
            outcome,
            Outcome::Cancelled {
                partial: "Half".into()
            }
        );
        priya_testkit::assert_events!(
            events(&s.store).await.iter(),
            [
                "intent.submitted",
                "model.called" => {"outcome": "cancelled"},
                "intent.cancelled" => {"reason": "stopped by the asker"},
            ]
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while s.fake.open_streams() > 0 {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::task::yield_now().await;
        }
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn a_missing_key_fails_before_any_call() {
        let s = setup_with(Vec::new(), Connect::NeedsKey);
        let (tx, _rx) = mpsc::channel(16);
        let outcome = ask(&s, tx, never_cancelled()).await;
        assert!(
            matches!(outcome, Outcome::Failed(ref e) if e.kind == ErrorKind::PermissionDenied
                && e.user_message.contains("priya ai key set gemini"))
        );
        priya_testkit::assert_events!(
            events(&s.store).await.iter(),
            ["intent.submitted", "intent.failed" => {"kind": "permission_denied"}]
        );
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn a_stop_while_waiting_for_the_keyring_needs_no_call() {
        let s = setup_with(Vec::new(), Connect::Hang);
        let (tx, _rx) = mpsc::channel(16);
        let (stop, cancel) = watch::channel(None);
        stop.send(Some(Stop::DaemonStopping)).unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(1), ask(&s, tx, cancel))
            .await
            .expect("the stop isn't held up by the keyring");
        assert_eq!(
            outcome,
            Outcome::Cancelled {
                partial: String::new()
            }
        );
        priya_testkit::assert_events!(
            events(&s.store).await.iter(),
            ["intent.submitted", "intent.cancelled"]
        );
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn an_error_after_the_first_token_records_when_it_came() {
        let s = setup(vec![Reply::Stream(vec![
            Step::Event(StreamEvent::Started { model: "m".into() }),
            Step::Event(StreamEvent::TextDelta("Half".into())),
            Step::Fail(ProviderError::new(
                ProviderErrorKind::Network,
                "The connection dropped.",
            )),
        ])]);
        let (tx, _rx) = mpsc::channel(16);
        let outcome = ask(&s, tx, never_cancelled()).await;
        assert!(
            matches!(outcome, Outcome::Failed(ref e) if e.user_message == "The connection dropped.")
        );
        let log = events(&s.store).await;
        priya_testkit::assert_events!(
            log.iter(),
            [
                "intent.submitted",
                "model.called" => {"outcome": "error", "error": "The connection dropped."},
                "intent.failed" => {"kind": "unavailable"},
            ]
        );
        assert!(log[1].payload["latency_first_token_ms"].is_u64());
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn an_asker_that_stops_reading_can_still_stop_the_answer() {
        let s = setup(vec![Reply::answer("m", &["one", "two", "three"])]);
        // One slot, never read: the second update can't be delivered.
        let (tx, _rx) = mpsc::channel(1);
        let (stop, cancel) = watch::channel(None);
        let answering = ask(&s, tx, cancel);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            stop.send(Some(Stop::Asked)).unwrap();
        };
        let (outcome, ()) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(answering, stopper)
        })
        .await
        .expect("a full channel doesn't block the stop");
        assert!(matches!(outcome, Outcome::Cancelled { .. }));
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn an_empty_answer_is_a_plain_failure() {
        let s = setup(vec![Reply::answer("m", &["  "])]);
        let (tx, _rx) = mpsc::channel(16);
        let outcome = ask(&s, tx, never_cancelled()).await;
        assert!(
            matches!(outcome, Outcome::Failed(ref e) if e.user_message.contains("empty answer"))
        );
        priya_testkit::assert_events!(
            events(&s.store).await.iter(),
            ["intent.submitted", "model.called" => {"outcome": "ok"}, "intent.failed" => {"kind": "provider_error"}]
        );
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn the_connector_builds_providers_and_reads_keys_for_the_answerer() {
        let connector = Connector::new(
            Arc::new(priya_ai::keys::MemorySecretStore::default()),
            priya_testkit::model_contract::TIMEOUTS,
        )
        .unwrap();
        let s = setup(Vec::new());
        let answerer = Answerer::new(
            Arc::new(connector),
            s.store.clone(),
            Arc::new(FakeClock::starting_at(datetime!(2026-10-20 12:00 UTC))),
            String::new(),
            UtcOffset::UTC,
        );
        let local = route(&ai(Some("fixed:local/m")), None).unwrap();
        assert!(answerer.providers().provider(&local).await.is_ok());
        assert!(!answerer.providers().key_stored("local").await.unwrap());
    }
}
