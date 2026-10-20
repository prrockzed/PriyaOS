//! `Reliable`'s timeouts and retry on the paths the contract suite doesn't
//! reach. Time is paused: the fake only sleeps on tokio's clock, so the
//! timeouts fire at once and exactly.

use std::sync::Arc;
use std::time::Duration;

use priya_ai::{
    Message, ModelProvider, ModelRequest, ProviderErrorKind, Reliable, StreamEvent, Timeouts,
};
use priya_testkit::{FakeModelProvider, Reply, Step};
use tokio_stream::StreamExt;

const TIMEOUTS: Timeouts = Timeouts {
    first_token: Duration::from_secs(60),
    idle: Duration::from_secs(30),
    total: Duration::from_secs(300),
};

fn reliable(fake: &Arc<FakeModelProvider>) -> Reliable {
    Reliable::new(fake.clone(), TIMEOUTS)
}

fn ask() -> ModelRequest {
    ModelRequest::new("fake-1", vec![Message::user("hi")])
}

fn started() -> Step {
    Step::Event(StreamEvent::Started {
        model: "fake-1".into(),
    })
}

// covers: AI-21
// suite: unit
#[tokio::test(start_paused = true)]
async fn no_text_within_the_first_token_timeout_is_a_timeout() {
    let fake =
        Arc::new(FakeModelProvider::new("fake").reply(Reply::Stream(vec![started(), Step::Hang])));
    let begun = tokio::time::Instant::now();
    let error = reliable(&fake).complete(ask()).await.unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::Timeout);
    assert_eq!(
        error.user_message,
        "The model took too long to start answering."
    );
    assert_eq!(begun.elapsed(), TIMEOUTS.first_token);
    assert_eq!(fake.requests().len(), 1, "a timeout isn't retried");
}

// covers: AI-21
// suite: unit
#[tokio::test(start_paused = true)]
async fn an_answer_longer_than_the_total_is_stopped() {
    // A chunk every 20 s never trips the idle timeout, only the total.
    let mut steps = vec![started()];
    for _ in 0..100 {
        steps.push(Step::Event(StreamEvent::TextDelta(".".into())));
        steps.push(Step::Pause(Duration::from_secs(20)));
    }
    let fake = Arc::new(FakeModelProvider::new("fake").reply(Reply::Stream(steps)));
    let begun = tokio::time::Instant::now();
    let events: Vec<_> = reliable(&fake)
        .complete(ask())
        .await
        .unwrap()
        .collect()
        .await;
    let error = events.last().unwrap().clone().unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::Timeout);
    assert_eq!(
        error.user_message,
        "The answer took too long, so it was stopped."
    );
    assert_eq!(begun.elapsed(), TIMEOUTS.total);
}

// covers: AI-21
// suite: unit
#[tokio::test(start_paused = true)]
async fn a_stream_that_ends_before_any_text_is_retried_and_started_comes_once() {
    let fake = Arc::new(
        FakeModelProvider::new("fake")
            .reply(Reply::Stream(vec![started()]))
            .reply(Reply::answer("fake-1", &["Hi"])),
    );
    let events: Vec<_> = reliable(&fake)
        .complete(ask())
        .await
        .unwrap()
        .collect()
        .await;
    let starts = events
        .iter()
        .filter(|e| matches!(e, Ok(StreamEvent::Started { .. })))
        .count();
    assert_eq!(starts, 1, "{events:?}");
    assert!(events.contains(&Ok(StreamEvent::TextDelta("Hi".into()))));
    assert_eq!(fake.requests().len(), 2);
}

// covers: AI-21
// suite: unit
#[tokio::test(start_paused = true)]
async fn cancelling_before_the_first_token_closes_the_stream() {
    let fake =
        Arc::new(FakeModelProvider::new("fake").reply(Reply::Stream(vec![started(), Step::Hang])));
    let provider = reliable(&fake);
    let waiting = tokio::time::timeout(Duration::from_secs(1), provider.complete(ask())).await;
    assert!(waiting.is_err(), "still waiting for text");
    tokio::task::yield_now().await;
    assert_eq!(fake.open_streams(), 0);
}
