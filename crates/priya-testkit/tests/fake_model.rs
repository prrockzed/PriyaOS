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

// covers: AI-21
// suite: unit
#[tokio::test(start_paused = true)]
async fn every_scenario_scripts_the_fake() {
    use priya_testkit::model_contract::{FakeSetup, Scenario, Setup};

    // Setups wrap the fake in `Reliable`, so being busy once is retried.
    for (scenario, first_error, requests) in [
        (Scenario::Answer, None, 1),
        (Scenario::Unauthorized, Some(ProviderErrorKind::Auth), 1),
        (
            Scenario::RateLimited,
            Some(ProviderErrorKind::RateLimited),
            1,
        ),
        (Scenario::OverloadedOnce, None, 2),
        (
            Scenario::OverloadedAlways,
            Some(ProviderErrorKind::Overloaded),
            2,
        ),
        (Scenario::FailsAfterFirstToken, None, 1),
        (Scenario::StallsAfterFirstToken, None, 1),
        (Scenario::Slow, None, 1),
    ] {
        let setup = FakeSetup::new(scenario).await;
        let provider = setup.provider();
        assert_eq!(provider.id(), "fake");
        match provider.complete(setup.request()).await {
            Err(error) => assert_eq!(Some(error.kind), first_error, "{scenario:?}"),
            Ok(mut stream) => {
                assert_eq!(first_error, None, "{scenario:?}");
                assert!(
                    matches!(stream.next().await, Some(Ok(StreamEvent::Started { .. }))),
                    "{scenario:?}"
                );
            }
        }
        assert_eq!(setup.requests(), requests, "{scenario:?}");
    }
}
