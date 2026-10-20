//! How fast appends are (sub-phase 0.5). Each event is its own append and
//! transaction, as in priyad. Informational: the 2 s budget for 10,000
//! events is enforced only in optimised builds and is measured on the
//! Zenbook with
//! `cargo nextest run --release -p priya-store -E 'binary(perf)'`;
//! debug builds on CI just report the time.

use std::sync::Arc;
use std::time::Instant;

use priya_core::clock::SystemClock;
use priya_store::{EventStore, StoreConfig};
use priya_testkit::events::setting_event;
use serde_json::Value;

// covers: CORE-01
// suite: perf
#[tokio::test]
async fn append_10k_events_under_2s() {
    let dir = tempfile::tempdir().unwrap();
    let store =
        EventStore::open(&StoreConfig::in_data_dir(dir.path()), Arc::new(SystemClock)).unwrap();
    let events: Vec<_> = (0..10_000)
        .map(|n| setting_event(&format!("perf.key{n}"), Value::from(n)))
        .collect();

    let started = Instant::now();
    for event in events {
        store.append(event).await.unwrap();
    }
    let took = started.elapsed();
    println!("appended 10,000 events one at a time in {took:?}");
    assert!(store.verify_chain(1..=i64::MAX).await.unwrap().ok);
    if !cfg!(debug_assertions) {
        assert!(took.as_secs_f64() < 2.0, "10,000 appends took {took:?}");
    }
}
