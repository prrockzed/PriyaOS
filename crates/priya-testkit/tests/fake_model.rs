//! The fake model provider plays exactly what it is scripted to.

use std::time::Duration;

use priya_ai::{
    Health, Message, ModelProvider, ModelRequest, ProviderError, ProviderErrorKind, StreamEvent,
};
use priya_testkit::{FakeModelProvider, Reply, Step};
use tokio_stream::StreamExt;

fn ask(text: &str) -> ModelRequest {
    ModelRequest::new("fake-1", vec![Message::user(text)])
}

// covers: AI-21
// suite: unit
#[tokio::test]
async fn the_fake_plays_its_replies_in_order_and_records_requests() {
    let busy = ProviderError::new(ProviderErrorKind::Overloaded, "Busy.");
    let fake = FakeModelProvider::new("fake")
        .reply(Reply::Refuse(busy.clone()))
        .reply(Reply::Stream(vec![
            Step::Event(StreamEvent::TextDelta("a".into())),
            Step::Fail(busy.clone()),
            Step::Event(StreamEvent::TextDelta("never sent".into())),
        ]));
    assert_eq!(fake.id(), "fake");

    // Nothing is used up until the future runs.
    drop(fake.complete(ask("unused")));
    assert!(fake.requests().is_empty());

    assert_eq!(fake.complete(ask("one")).await.unwrap_err(), busy);
    let events: Vec<_> = fake.complete(ask("two")).await.unwrap().collect().await;
    assert_eq!(
        events,
        vec![Ok(StreamEvent::TextDelta("a".into())), Err(busy)]
    );
    let none_left = fake.complete(ask("three")).await.unwrap_err();
    assert_eq!(none_left.kind, ProviderErrorKind::BadRequest);

    let asked: Vec<String> = fake
        .requests()
        .iter()
        .map(|r| r.messages[0].text.clone())
        .collect();
    assert_eq!(asked, ["one", "two", "three"]);
}

// covers: AI-21
// suite: unit
#[tokio::test]
async fn the_fake_reports_its_health_and_closes_dropped_streams() {
    let fake = FakeModelProvider::new("fake").reply(Reply::Stream(vec![
        Step::Event(StreamEvent::TextDelta("a".into())),
        Step::Hang,
    ]));
    assert_eq!(fake.health().await, Health::Ok);
    let down = ProviderError::new(ProviderErrorKind::Network, "Unreachable.");
    fake.set_health(Health::Down(down.clone()));
    assert_eq!(fake.health().await, Health::Down(down));

    let mut stream = fake.complete(ask("hi")).await.unwrap();
    assert!(stream.next().await.is_some());
    assert_eq!(fake.open_streams(), 1);
    drop(stream);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    while fake.open_streams() > 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the stream stayed open"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
