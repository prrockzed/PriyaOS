//! The types of one question and the events that record it (02-core
//! §1.1), in this order:
//!
//! - `intent.submitted` first, by the user;
//! - `model.called` once a provider was asked, by Priya AI;
//! - then exactly one of `intent.answered`, `intent.cancelled` (by whoever
//!   stopped it) or `intent.failed`.
//!
//! All of an intent's events are on the stream `intent:<id>`, correlated
//! with `conversation:<id>`, and caused by the `intent.submitted` event.

use std::time::Duration;

use priya_ai::gateway::Route;
use priya_ai::{StreamEvent, Usage};
use priya_api::error::ApiError;
use priya_api::events::{
    Actor, CallOutcome, EventPayload, IntentAnswered, IntentCancelled, IntentFailed,
    IntentSubmitted, ModelCalled, NewEvent,
};
use priya_core::clock::Clock;
use priya_store::EventStore;
use tracing::warn;
use ulid::Ulid;

/// What the model is called for, in `model.called`, when no role is given.
pub const ROLE: &str = "answer";

/// One question to answer.
#[derive(Debug, Clone)]
pub struct Question {
    pub id: Ulid,
    pub text: String,
    pub conversation: String,
    /// Where it was asked: `cli`, `bar`, a module.
    pub surface: String,
    /// A `<provider>/<model>` that overrides `ai.strategy`.
    pub model: Option<String>,
    /// What it is for (`fast`, `smart`…), recorded in `model.called`;
    /// every role uses `ai.strategy` until 1.7.
    pub role: Option<String>,
}

/// What the asker sees while the answer streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// The model accepted the question.
    Started {
        provider: String,
        model: String,
        local: bool,
    },
    Delta(String),
    Usage(Usage),
}

/// Why an answer was stopped before its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The asker said so (`v1.intent.cancel`, Esc, Ctrl+C).
    Asked,
    /// The asking connection closed: nobody would read the rest.
    ConnectionClosed,
    /// priyad is stopping.
    DaemonStopping,
}

impl Stop {
    /// Who stopped it, for `intent.cancelled`.
    pub fn actor(self) -> Actor {
        match self {
            Self::Asked => Actor::User,
            Self::ConnectionClosed | Self::DaemonStopping => Actor::System,
        }
    }

    pub fn reason(self) -> &'static str {
        match self {
            Self::Asked => "stopped by the asker",
            Self::ConnectionClosed => "the asking connection closed",
            Self::DaemonStopping => "priyad is stopping",
        }
    }
}

/// How the question ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Answered {
        text: String,
        model: String,
    },
    /// Stopped (by the asker, a closed connection or priyad stopping);
    /// `partial` is what had arrived.
    Cancelled {
        partial: String,
    },
    Failed(ApiError),
}

/// One model call as it goes, for `model.called`.
pub struct Call {
    pub provider: String,
    role: String,
    /// The model asked for, until the provider names the one answering.
    pub model: String,
    began: std::time::Instant,
    first_token: Option<Duration>,
    usage: Option<Usage>,
    pub text: String,
}

impl Call {
    pub fn start(clock: &dyn Clock, route: &Route, role: Option<&str>) -> Self {
        Self {
            provider: route.provider.as_str().to_owned(),
            role: role.unwrap_or(ROLE).to_owned(),
            model: route.model.clone(),
            began: clock.monotonic(),
            first_token: None,
            usage: None,
            text: String::new(),
        }
    }

    /// Notes a stream event; what the asker should see, if anything.
    pub fn take(&mut self, clock: &dyn Clock, event: StreamEvent, route: &Route) -> Option<Update> {
        match event {
            StreamEvent::Started { model } => {
                self.model = model.clone();
                Some(Update::Started {
                    provider: self.provider.clone(),
                    model,
                    local: route.local,
                })
            }
            StreamEvent::TextDelta(text) => {
                if self.first_token.is_none() {
                    self.first_token = Some(clock.monotonic() - self.began);
                }
                self.text.push_str(&text);
                Some(Update::Delta(text))
            }
            StreamEvent::Usage(usage) => {
                // A provider counting as it goes: the last count wins.
                self.usage = Some(usage);
                Some(Update::Usage(usage))
            }
            _ => None,
        }
    }

    pub fn finish(
        &self,
        clock: &dyn Clock,
        outcome: CallOutcome,
        error: Option<&ApiError>,
    ) -> ModelCalled {
        // No usage reported (a stopped or failed call) is written as 0:
        // model.called v1 has no way to say "unknown".
        let usage = self.usage.unwrap_or_default();
        let millis = |d: Duration| u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
        ModelCalled {
            provider: self.provider.clone(),
            model: self.model.clone(),
            role: self.role.clone(),
            tokens_in: usage.input,
            tokens_out: usage.output,
            tokens_cached: usage.cached.unwrap_or(0),
            cost_micro_usd: 0,
            latency_ms: millis(clock.monotonic() - self.began),
            latency_first_token_ms: self.first_token.map(millis),
            outcome,
            error: error.map(|e| e.user_message.clone()),
        }
    }
}

/// Writes one intent's events with the right stream, actor and links.
pub struct IntentLog<'a> {
    store: &'a EventStore,
    stream: String,
    correlation: String,
    /// The `intent.submitted` event, which caused the rest.
    cause: std::sync::Mutex<Option<Ulid>>,
}

impl<'a> IntentLog<'a> {
    pub fn new(store: &'a EventStore, question: &Question) -> Self {
        Self {
            store,
            stream: format!("intent:{}", question.id),
            correlation: format!("conversation:{}", question.conversation),
            cause: std::sync::Mutex::new(None),
        }
    }

    pub async fn submitted(&self, question: &Question) {
        let submitted = IntentSubmitted {
            text: question.text.clone(),
            conversation: Some(question.conversation.clone()),
            surface: question.surface.clone(),
        };
        let id = self.record(Actor::User, &submitted).await;
        *self.cause.lock().unwrap_or_else(|e| e.into_inner()) = id;
    }

    pub async fn called(&self, call: ModelCalled) {
        self.record(Actor::PriyaAi, &call).await;
    }

    pub async fn answered(&self, text: String, model: String) -> Outcome {
        let answered = IntentAnswered {
            answer: text.clone(),
            model: model.clone(),
        };
        self.record(Actor::PriyaAi, &answered).await;
        Outcome::Answered { text, model }
    }

    pub async fn cancelled(&self, stop: Stop, partial: String) -> Outcome {
        let reason = Some(stop.reason().to_owned());
        self.record(stop.actor(), &IntentCancelled { reason }).await;
        Outcome::Cancelled { partial }
    }

    pub async fn failed(&self, error: ApiError) -> Outcome {
        let failed = IntentFailed {
            kind: error.kind,
            error: error.user_message.clone(),
        };
        self.record(Actor::PriyaAi, &failed).await;
        Outcome::Failed(error)
    }

    /// Appends an event; a store that fails is logged, not fatal: the
    /// asker still gets the answer.
    async fn record<P: EventPayload>(&self, actor: Actor, payload: &P) -> Option<Ulid> {
        let cause = *self.cause.lock().unwrap_or_else(|e| e.into_inner());
        let event = match NewEvent::new(self.stream.clone(), actor, payload) {
            Ok(event) => event.correlated(self.correlation.clone()),
            Err(e) => {
                warn!(error = %e, event = P::TYPE, "can't build an intent event");
                return None;
            }
        };
        let event = match cause {
            Some(cause) => event.caused_by(cause),
            None => event,
        };
        match self.store.append(event).await {
            Ok(record) => Some(record.event.id),
            Err(e) => {
                warn!(error = %e, event = P::TYPE, "can't record an intent event");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use priya_ai::FinishReason;
    use priya_api::events::EventEnvelope;
    use priya_core::settings::ProviderName;
    use priya_store::StoreConfig;
    use priya_testkit::FakeClock;

    use super::*;

    fn route() -> Route {
        Route {
            provider: ProviderName::try_from("local".to_owned()).unwrap(),
            model: "m".into(),
            base_url: "http://127.0.0.1:11434/v1".into(),
            local: true,
            label: "the local model server".into(),
            reasoning_effort: None,
        }
    }

    fn question() -> Question {
        Question {
            id: Ulid::generate(),
            text: "hi".into(),
            conversation: "c1".into(),
            surface: "cli".into(),
            model: None,
            role: None,
        }
    }

    // covers: AI-21, CORE-01
    // suite: unit
    #[tokio::test]
    async fn an_intents_events_share_a_stream_and_point_to_the_question() {
        let dir = tempfile::tempdir().unwrap();
        let clock = Arc::new(FakeClock::new());
        let store = EventStore::open(&StoreConfig::in_data_dir(dir.path()), clock.clone()).unwrap();
        let first = question();
        let log = IntentLog::new(&store, &first);
        log.submitted(&first).await;
        let call = Call::start(&*clock, &route(), None);
        log.called(call.finish(&*clock, CallOutcome::Cancelled, None))
            .await;
        let outcome = log.cancelled(Stop::ConnectionClosed, "Half".into()).await;
        assert_eq!(
            outcome,
            Outcome::Cancelled {
                partial: "Half".into()
            }
        );

        let events: Vec<EventEnvelope> = store
            .read_after(0, 10)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.event)
            .collect();
        let actors: Vec<String> = events.iter().map(|e| e.actor.to_string()).collect();
        assert_eq!(actors, ["user", "priya-ai", "system"]);
        let stream = format!("intent:{}", first.id);
        assert!(events.iter().all(|e| e.stream == stream));
        assert!(
            events
                .iter()
                .all(|e| e.correlation.as_deref() == Some("conversation:c1"))
        );
        assert!(
            events[1..]
                .iter()
                .all(|e| e.causation == Some(events[0].id))
        );
        assert_eq!(events[2].payload["reason"], "the asking connection closed");
        assert_eq!(Stop::Asked.actor(), Actor::User);
        assert_eq!(Stop::Asked.reason(), "stopped by the asker");

        // The other two endings: an answer, and a failure.
        let answered = question();
        let log = IntentLog::new(&store, &answered);
        log.submitted(&answered).await;
        let outcome = log.answered("Hello.".into(), "m-1".into()).await;
        assert_eq!(
            outcome,
            Outcome::Answered {
                text: "Hello.".into(),
                model: "m-1".into()
            }
        );
        let failed = question();
        let log = IntentLog::new(&store, &failed);
        log.submitted(&failed).await;
        let error = ApiError::new(
            priya_api::error::ErrorKind::ProviderError,
            "Gemini is busy right now.",
        );
        assert_eq!(log.failed(error.clone()).await, Outcome::Failed(error));
        let last: Vec<EventEnvelope> = store
            .read_after(3, 10)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.event)
            .collect();
        let types: Vec<&str> = last.iter().map(|e| e.event_type.as_str()).collect();
        assert_eq!(
            types,
            [
                "intent.submitted",
                "intent.answered",
                "intent.submitted",
                "intent.failed"
            ]
        );
        assert_eq!(last[1].payload["answer"], "Hello.");
        assert_eq!(last[3].payload["kind"], "provider_error");
        assert_eq!(last[3].causation, Some(last[2].id));
        assert_eq!(Stop::DaemonStopping.reason(), "priyad is stopping");
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn a_call_measures_its_first_token_and_its_whole_length() {
        let clock = FakeClock::new();
        let mut call = Call::start(&clock, &route(), Some("fast"));
        clock.advance(Duration::from_millis(40));
        let started = call.take(
            &clock,
            StreamEvent::Started {
                model: "m-1".into(),
            },
            &route(),
        );
        assert_eq!(
            started,
            Some(Update::Started {
                provider: "local".into(),
                model: "m-1".into(),
                local: true
            })
        );
        clock.advance(Duration::from_millis(60));
        assert_eq!(
            call.take(&clock, StreamEvent::TextDelta("Hi".into()), &route()),
            Some(Update::Delta("Hi".into()))
        );
        clock.advance(Duration::from_millis(900));
        call.take(&clock, StreamEvent::TextDelta("!".into()), &route());
        let finished = StreamEvent::Finished {
            reason: FinishReason::Stop,
        };
        assert_eq!(call.take(&clock, finished, &route()), None);

        let called = call.finish(&clock, CallOutcome::Ok, None);
        assert_eq!(called.model, "m-1", "the model that answered");
        assert_eq!(called.role, "fast");
        assert_eq!(called.latency_first_token_ms, Some(100));
        assert_eq!(called.latency_ms, 1000);
        assert_eq!(
            (called.tokens_in, called.tokens_out, called.tokens_cached),
            (0, 0, 0),
            "no usage reported"
        );
        assert_eq!(call.text, "Hi!");

        // Usage: the last count wins; no role means "answer".
        let mut counted = Call::start(&clock, &route(), None);
        counted.take(&clock, StreamEvent::Usage(Usage::new(1, 1)), &route());
        counted.take(
            &clock,
            StreamEvent::Usage(Usage::new(5, 7).with_cached(3)),
            &route(),
        );
        let error = ApiError::new(priya_api::error::ErrorKind::Timeout, "Too slow.");
        let failed = counted.finish(&clock, CallOutcome::Error, Some(&error));
        assert_eq!(
            (failed.tokens_in, failed.tokens_out, failed.tokens_cached),
            (5, 7, 3)
        );
        assert_eq!(failed.role, "answer");
        assert_eq!(failed.error.as_deref(), Some("Too slow."));
        assert_eq!(failed.latency_first_token_ms, None);
    }
}
