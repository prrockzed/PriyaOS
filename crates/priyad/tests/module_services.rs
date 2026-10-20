//! What the module host reaches in the daemon (0.8): events, settings and
//! the public API, through [`DaemonServices`] over a daemon's parts in
//! temporary folders.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::machine;
use priya_api::error::ErrorKind;
use priya_api::events::{Actor, NewEvent};
use priya_core::settings::KeyPath;
use priyad::api::router;
use priyad::module_api::DaemonServices;
use priyad::modules::host_api::HostServices;
use serde_json::{Value, json};

/// What `host.emit_event` records for a module named `test`.
fn event(text: &str) -> NewEvent {
    NewEvent {
        event_type: "module.event".to_owned(),
        version: 1,
        stream: "module:test".to_owned(),
        correlation: None,
        causation: None,
        actor: Actor::Module("test".to_owned()),
        payload: json!({ "module": "test", "name": "said", "data": { "text": text } }),
    }
}

// covers: CORE-02
// suite: integration
#[tokio::test]
async fn modules_reach_events_settings_and_the_api_until_priyad_stops() {
    let machine = machine().await;
    let services = DaemonServices {
        daemon: Arc::downgrade(&machine.daemon),
        router: Arc::new(router()),
    };

    // Only events from now on, in full.
    let before = services.record(event("old")).await.expect("recorded");
    let mut events = services.subscribe(vec!["module.*".to_owned()]);
    // The subscription starts in its own task; give it a moment.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let seq = services.record(event("hello")).await.expect("recorded");
    assert!(seq > before);
    let got = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("an event in time")
        .expect("the channel is open");
    assert_eq!(got.seq, seq);
    assert_eq!(got.event.payload["data"], json!({ "text": "hello" }));

    // Settings in force; null when not set.
    let enabled = services.setting(&KeyPath::from_parts(["modules", "enabled"]));
    assert!(enabled.is_array(), "{enabled}");
    assert_eq!(
        services.setting(&KeyPath::from_parts(["no", "such"])),
        Value::Null
    );

    // The public API, and its errors as they are.
    let status = services
        .call_api("v1.system.status".to_owned(), json!({}))
        .await
        .expect("status answers");
    assert!(status.get("version").is_some(), "{status}");
    let error = services
        .call_api("v1.no.such".to_owned(), json!({}))
        .await
        .expect_err("no such method");
    assert_eq!(error.kind, ErrorKind::NotFound);

    // A module whose side goes away ends its subscription.
    drop(events);

    // Once priyad is gone, nothing more is reached and nothing hangs.
    drop(machine);
    assert_eq!(
        services
            .record(event("late"))
            .await
            .expect_err("priyad is gone")
            .kind,
        ErrorKind::Unavailable
    );
    assert_eq!(
        services
            .call_api("v1.system.status".to_owned(), json!({}))
            .await
            .expect_err("priyad is gone")
            .kind,
        ErrorKind::Unavailable
    );
    assert_eq!(
        services.setting(&KeyPath::from_parts(["modules", "enabled"])),
        Value::Null
    );
    let mut late = services.subscribe(vec!["*".to_owned()]);
    assert!(late.recv().await.is_none(), "closed at once");
}
