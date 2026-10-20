//! `model_provider_contract` (0.12): what every model provider must do,
//! written once and run against each one (the fake, the OpenAI-compatible
//! adapter on a scripted server, recorded cassettes).
//!
//! A subject turns a [`Scenario`] into a [`Setup`]: a provider and a view
//! of what reached it. Each check below takes a function making
//! setups, so a test is one line:
//!
//! ```ignore
//! contract::streams_text_deltas_in_order(fake_setup).await;
//! ```

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use priya_ai::{
    FinishReason, ModelProvider, ModelRequest, ModelStream, ProviderError, ProviderErrorKind,
    StreamEvent,
};
use tokio_stream::StreamExt;

use crate::model::{FakeModelProvider, Reply, Step};

/// The answer [`Scenario::Answer`] streams, in these chunks.
pub const CHUNKS: [&str; 3] = ["Hello", " world", "!"];
/// What [`Scenario::RateLimited`] asks us to wait.
pub const RETRY_AFTER: Duration = Duration::from_secs(7);

/// What the provider is set up to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    /// Answers with [`CHUNKS`], then usage.
    Answer,
    /// Refuses the key (HTTP 401).
    Unauthorized,
    /// Rate-limits, asking to wait [`RETRY_AFTER`] (HTTP 429).
    RateLimited,
    /// Busy once (HTTP 503), then answers.
    OverloadedOnce,
    /// Busy for every request.
    OverloadedAlways,
    /// Sends one chunk, then fails.
    FailsAfterFirstToken,
    /// Sends one chunk, then nothing at all, so the idle timeout fires
    /// whatever it is set to.
    StallsAfterFirstToken,
    /// Sends a chunk every 100 ms for a long time.
    Slow,
    /// Sends a line that isn't valid streaming data (HTTP adapters only).
    Malformed,
}

/// A provider set up for one scenario. A subject that can't produce a
/// scenario (a recording has no rate limit in it) doesn't run the checks
/// that need it, rather than faking a pass.
pub trait Setup {
    fn provider(&self) -> Arc<dyn ModelProvider>;
    /// How many requests reached the provider.
    fn requests(&self) -> usize;
    /// Answers still being produced (open connections for HTTP). Stopping
    /// takes effect a moment after a drop, so checks poll it.
    fn open(&self) -> usize;
    /// What the checks ask; a recording replays only the request it saw.
    fn request(&self) -> ModelRequest {
        ModelRequest::new("contract-model", vec![priya_ai::Message::user("hi")])
            .with_max_output_tokens(16)
    }
    /// The full text of [`Scenario::Answer`].
    fn expected_answer(&self) -> String {
        CHUNKS.concat()
    }
}

async fn start<S: Setup>(setup: &S) -> Result<ModelStream, ProviderError> {
    setup.provider().complete(setup.request()).await
}

async fn collect(stream: ModelStream) -> Vec<Result<StreamEvent, ProviderError>> {
    stream.collect().await
}

fn text(events: &[Result<StreamEvent, ProviderError>]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            Ok(StreamEvent::TextDelta(text)) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

pub async fn streams_text_deltas_in_order<S: Setup, F: Future<Output = S>>(
    make: impl FnOnce(Scenario) -> F,
) {
    let setup = make(Scenario::Answer).await;
    let events = collect(start(&setup).await.expect("the answer starts")).await;
    assert!(
        matches!(events.first(), Some(Ok(StreamEvent::Started { .. }))),
        "{events:?}"
    );
    assert_eq!(text(&events), setup.expected_answer());
    assert!(events.iter().all(Result::is_ok), "{events:?}");
    assert!(events.contains(&Ok(StreamEvent::Finished {
        reason: FinishReason::Stop
    })));
}

pub async fn reports_usage_at_end<S: Setup, F: Future<Output = S>>(
    make: impl FnOnce(Scenario) -> F,
) {
    let setup = make(Scenario::Answer).await;
    let events = collect(start(&setup).await.expect("the answer starts")).await;
    let usage = events
        .iter()
        .position(|e| matches!(e, Ok(StreamEvent::Usage(_))))
        .expect("usage is reported");
    let last_text = events
        .iter()
        .rposition(|e| matches!(e, Ok(StreamEvent::TextDelta(_))))
        .expect("there is text");
    assert!(usage > last_text, "usage comes after the text: {events:?}");
    let Ok(StreamEvent::Usage(usage)) = &events[usage] else {
        unreachable!()
    };
    assert!(usage.input > 0 && usage.output > 0, "{usage:?}");
}

/// The fake as a contract subject.
pub struct FakeSetup(Arc<FakeModelProvider>);

impl FakeSetup {
    /// [`FakeModelProvider`] scripted for `scenario`.
    pub async fn new(scenario: Scenario) -> Self {
        let error = |kind, message: &str| ProviderError::new(kind, message);
        let busy = || {
            error(
                ProviderErrorKind::Overloaded,
                "The fake model is busy right now.",
            )
        };
        let answer = || Reply::answer("fake-1", &CHUNKS);
        let delta = |text: &str| Step::Event(StreamEvent::TextDelta(text.to_owned()));
        let started = || {
            Step::Event(StreamEvent::Started {
                model: "fake-1".into(),
            })
        };
        let replies = match scenario {
            Scenario::Answer => vec![answer()],
            Scenario::Unauthorized => vec![Reply::Refuse(error(
                ProviderErrorKind::Auth,
                "The key for the fake model was refused.",
            ))],
            Scenario::RateLimited => vec![Reply::Refuse(
                error(
                    ProviderErrorKind::RateLimited,
                    "The fake model is rate-limited.",
                )
                .with_retry_after(Some(RETRY_AFTER)),
            )],
            Scenario::OverloadedOnce => vec![Reply::Refuse(busy()), answer()],
            Scenario::OverloadedAlways => (0..5).map(|_| Reply::Refuse(busy())).collect(),
            Scenario::FailsAfterFirstToken => vec![Reply::Stream(vec![
                started(),
                delta("Hel"),
                Step::Fail(busy()),
            ])],
            Scenario::StallsAfterFirstToken => {
                vec![Reply::Stream(vec![started(), delta("Hel"), Step::Hang])]
            }
            Scenario::Slow => {
                let mut steps = vec![started()];
                for _ in 0..100 {
                    steps.push(delta("."));
                    steps.push(Step::Pause(Duration::from_millis(100)));
                }
                vec![Reply::Stream(steps)]
            }
            Scenario::Malformed => panic!("the fake has no wire format to get wrong"),
        };
        let fake = FakeModelProvider::new("fake");
        for reply in replies {
            fake.push(reply);
        }
        Self(Arc::new(fake))
    }
}

impl Setup for FakeSetup {
    fn provider(&self) -> Arc<dyn ModelProvider> {
        self.0.clone()
    }

    fn requests(&self) -> usize {
        self.0.requests().len()
    }

    fn open(&self) -> usize {
        self.0.open_streams()
    }
}
