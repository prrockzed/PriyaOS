//! Crash safety under a hard kill (sub-phase 0.5, CORE-01): a process
//! appending in a loop is killed with SIGKILL, three times. Afterwards the log
//! must verify, hold no partial row, and still contain every event the store
//! acknowledged before the kill.
//!
//! SIGKILL leaves the kernel's page cache intact, so this proves that commits
//! are all or nothing and that an acknowledged commit outlives the process.
//! Surviving a power cut (`synchronous=FULL`) needs a VM test.
//!
//! The appending process is this test binary again, running the ignored
//! `append_loop_child` (see [`priya_testkit::LoopChild`]). It acknowledges
//! the last seq of each batch.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use priya_core::clock::SystemClock;
use priya_store::{EventStore, StoreConfig};
use priya_testkit::LoopChild;
use priya_testkit::events::setting_event;
use serde_json::Value;

const CHILD_DIR: &str = "PRIYA_APPEND_LOOP_DIR";
/// Batches the child must acknowledge before it may be killed, so every
/// round tests something however slow the machine is.
const ACKS_BEFORE_KILL: usize = 3;

/// Every tenth payload is big enough to go to the blob store.
fn event(n: u64) -> priya_api::events::NewEvent {
    let value = if n.is_multiple_of(10) {
        "x".repeat(10_000)
    } else {
        n.to_string()
    };
    setting_event(&format!("crash.key{n}"), Value::from(value))
}

fn highest(acks: &[String]) -> i64 {
    acks.iter()
        .map(|seq| seq.parse::<i64>().expect("the child acks seqs"))
        .max()
        .unwrap_or(0)
}

// covers: CORE-01
// suite: integration
// acceptance: 0.5
// regression: 0.5
#[test]
fn events_survive_kill_during_append_loop() {
    let dir = tempfile::tempdir().unwrap();
    let mut acknowledged = 0;
    for round in 0..3_u64 {
        let mut child = LoopChild::spawn(
            "append_loop_child",
            &[(CHILD_DIR, dir.path().as_os_str())],
            dir.path(),
        );
        let mut acks = child.wait_for_acks(ACKS_BEFORE_KILL);
        // Vary where in a batch the kill lands.
        std::thread::sleep(Duration::from_millis(round * 7));
        acks.extend(child.kill());
        let highest = highest(&acks);
        assert!(highest > acknowledged, "round {round} appended nothing new");
        acknowledged = highest;
    }

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let config = StoreConfig::in_data_dir(dir.path());
        let store = EventStore::open(&config, Arc::new(SystemClock)).unwrap();
        let report = store.verify_chain(1..=i64::MAX).await.unwrap();
        assert!(report.ok, "{report:?}");

        // No partial rows and nothing acknowledged lost: seqs run 1..=n with
        // n at least the last acknowledged seq, and every blob is there.
        let records = store.read_after(0, usize::MAX >> 1).await.unwrap();
        assert_eq!(records.len() as u64, report.checked);
        assert!(
            records.len() as i64 >= acknowledged,
            "{} events stored, but {acknowledged} were acknowledged",
            records.len()
        );
        for (i, record) in records.iter().enumerate() {
            assert_eq!(record.seq, i as i64 + 1);
            store.payload(record).await.unwrap();
        }
        // And the log still takes new events.
        store.append(event(0)).await.unwrap();
        assert!(store.verify_chain(1..=i64::MAX).await.unwrap().ok);
    });
}

// covers: infra
// suite: integration
#[test]
#[ignore = "helper process for events_survive_kill_during_append_loop"]
fn append_loop_child() {
    let Some(dir) = std::env::var_os(CHILD_DIR) else {
        return;
    };
    let stop = priya_testkit::until_stdin_closes();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let store = EventStore::open(
            &StoreConfig::in_data_dir(std::path::Path::new(&dir)),
            Arc::new(SystemClock),
        )
        .unwrap();
        let mut n = 0_u64;
        while !stop.load(Ordering::Relaxed) {
            n += 1;
            let records = store
                .append_all((0..5).map(|i| event(n * 5 + i)).collect())
                .await
                .unwrap();
            priya_testkit::ack(records.last().unwrap().seq);
        }
    });
}
