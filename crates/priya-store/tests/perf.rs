//! How fast appends are (sub-phase 0.5). Informational: the 2 s budget for
//! 10,000 events is enforced only in optimised builds, measured on the
//! Zenbook; debug builds on CI just report the time.

use std::sync::Arc;
use std::time::Instant;

use priya_api::events::{Actor, NewEvent, SettingChanged, SettingSource};
use priya_core::clock::SystemClock;
use priya_store::{EventStore, StoreConfig};
use serde_json::Value;

// covers: CORE-01
// suite: perf
#[tokio::test]
async fn append_10k_events_under_2s() {
    let dir = tempfile::tempdir().unwrap();
    let store =
        EventStore::open(&StoreConfig::in_data_dir(dir.path()), Arc::new(SystemClock)).unwrap();
    let events: Vec<NewEvent> = (0..10_000)
        .map(|n| {
            NewEvent::new(
                "settings",
                Actor::System,
                &SettingChanged {
                    key: format!("perf.key{n}"),
                    old: None,
                    new: Some(Value::from(n)),
                    source: SettingSource::File,
                },
            )
            .unwrap()
        })
        .collect();

    let started = Instant::now();
    for batch in events.chunks(100) {
        store.append_all(batch.to_vec()).await.unwrap();
    }
    let took = started.elapsed();
    println!("appended 10,000 events in {took:?}");
    assert!(store.verify_chain(1..=i64::MAX).await.unwrap().ok);
    if !cfg!(debug_assertions) {
        assert!(took.as_secs_f64() < 2.0, "10,000 appends took {took:?}");
    }
}
