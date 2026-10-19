//! Durability under a hard kill (sub-phase 0.5, CORE-01): a process appending
//! in a loop is killed with SIGKILL, several times. Afterwards the log must
//! verify and contain no partial row: every append is all or nothing, and
//! `synchronous=FULL` makes an acknowledged commit survive.
//!
//! The appending process is this test binary again, running the ignored
//! `append_loop_child` helper.

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use priya_api::events::{Actor, NewEvent, SettingChanged, SettingSource};
use priya_core::clock::SystemClock;
use priya_store::{EventStore, StoreConfig};
use serde_json::Value;

const CHILD_DIR: &str = "PRIYA_APPEND_LOOP_DIR";

fn event(n: u64) -> NewEvent {
    NewEvent::new(
        "settings",
        Actor::System,
        &SettingChanged {
            key: format!("crash.key{n}"),
            old: None,
            // Every tenth payload is big enough to go to the blob store.
            new: Some(Value::from(if n.is_multiple_of(10) {
                "x".repeat(10_000)
            } else {
                n.to_string()
            })),
            source: SettingSource::File,
        },
    )
    .expect("a SettingChanged always serialises")
}

// covers: CORE-01
// suite: integration
// acceptance: 0.5
#[test]
fn events_survive_kill_during_append_loop() {
    let dir = tempfile::tempdir().unwrap();
    let me = std::env::current_exe().unwrap();
    for round in 0..3_u64 {
        let mut child = Command::new(&me)
            .args([
                "append_loop_child",
                "--exact",
                "--include-ignored",
                "--test-threads=1",
            ])
            .env(CHILD_DIR, dir.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(400 + round * 150));
        child.kill().unwrap(); // SIGKILL on Unix: no chance to clean up
        child.wait().unwrap();
    }

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let config = StoreConfig::in_data_dir(dir.path());
        let store = EventStore::open(&config, Arc::new(SystemClock)).unwrap();
        let report = store.verify_chain(1..=i64::MAX).await.unwrap();
        assert!(report.ok, "{report:?}");
        assert!(
            report.checked > 0,
            "the child appended something before each kill"
        );

        // No partial rows: every stored event reads back whole, with seqs
        // 1..=n and each large payload's blob present.
        let records = store.read_after(0, usize::MAX >> 1).await.unwrap();
        assert_eq!(records.len() as u64, report.checked);
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
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let store = EventStore::open(
            &StoreConfig::in_data_dir(std::path::Path::new(&dir)),
            Arc::new(SystemClock),
        )
        .unwrap();
        // Until killed.
        let mut n = 0_u64;
        loop {
            n += 1;
            store
                .append_all((0..5).map(|i| event(n * 5 + i)).collect())
                .await
                .unwrap();
        }
    });
}
