//! Subscriptions: a filtered stream of events, from a given point on
//! (02-core §1.3). This is how modules and surfaces learn what happened.
//!
//! A subscription always reads events from the database. After each commit
//! the writer publishes the new head seq on a `watch` channel, which only
//! ever holds the latest value; the subscription wakes up and reads what it
//! hasn't seen yet. So a slow subscriber is simply further behind in the
//! database, and no event is ever dropped. Delivery is in seq order, hence
//! ordered per stream, and at least once: a consumer stores the last seq it
//! handled and resumes after it.
//!
//! The task holds only the store's read side. After the store is dropped, a
//! task blocked on a full buffer keeps its read connections until the
//! consumer reads on or drops the stream.

use std::sync::Arc;

use futures_core::Stream;
use tokio::sync::{mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;

use crate::error::Result;
use crate::store::{EventRecord, Reader};

/// Events read from the database per round trip while catching up.
const BATCH: usize = 512;
/// Events a subscriber may have waiting before the reader pauses.
const BUFFER: usize = 256;

/// Which events a subscriber wants.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Type patterns where `*` matches anything, such as `intent.*` or
    /// `module.crashed`. Empty means every type.
    pub types: Vec<String>,
    /// Only streams starting with this, such as `intent:`.
    pub stream_prefix: Option<String>,
}

impl Filter {
    /// Every event.
    pub fn all() -> Self {
        Self::default()
    }

    pub fn matches(&self, record: &EventRecord) -> bool {
        let type_ok = self.types.is_empty()
            || self
                .types
                .iter()
                .any(|pattern| glob_match(pattern, &record.event.event_type));
        let stream_ok = self
            .stream_prefix
            .as_deref()
            .is_none_or(|prefix| record.event.stream.starts_with(prefix));
        type_ok && stream_ok
    }
}

/// `*` matches any run of characters, including none.
fn glob_match(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => {
            text.starts_with(head)
                && (head.len()..=text.len())
                    .filter(|&i| text.is_char_boundary(i))
                    .any(|i| glob_match(tail, &text[i..]))
        }
    }
}

/// Starts a subscription task; see [`crate::EventStore::subscribe`]. The
/// task holds only the read side, so it ends once the store is dropped.
pub(crate) fn spawn(
    reader: Arc<Reader>,
    mut head: watch::Receiver<i64>,
    filter: Filter,
    after: i64,
) -> impl Stream<Item = Result<EventRecord>> {
    let (sender, receiver) = mpsc::channel(BUFFER);
    tokio::spawn(async move {
        let mut last = after;
        loop {
            // Mark the current head as seen before reading, so a commit that
            // lands during the read still wakes us afterwards.
            head.borrow_and_update();
            loop {
                let batch = match reader.read_after(last, BATCH).await {
                    Ok(batch) => batch,
                    Err(e) => {
                        let _ = sender.send(Err(e)).await;
                        return;
                    }
                };
                if batch.is_empty() {
                    break;
                }
                for record in batch {
                    last = record.seq;
                    if filter.matches(&record) && sender.send(Ok(record)).await.is_err() {
                        return; // the subscriber went away
                    }
                }
            }
            tokio::select! {
                changed = head.changed() => {
                    if changed.is_err() {
                        return; // the store shut down
                    }
                }
                () = sender.closed() => return,
            }
        }
    });
    ReceiverStream::new(receiver)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use priya_api::events::{Actor, IntentSubmitted, ModuleStarted, NewEvent};
    use priya_testkit::FakeClock;
    use tokio_stream::StreamExt;

    use super::*;
    use crate::store::{EventStore, StoreConfig};

    fn store() -> (tempfile::TempDir, EventStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = EventStore::open(
            &StoreConfig::in_data_dir(dir.path()),
            Arc::new(FakeClock::new()),
        )
        .unwrap();
        (dir, store)
    }

    fn intent(id: u32) -> NewEvent {
        NewEvent::new(
            format!("intent:{id}"),
            Actor::User,
            &IntentSubmitted {
                text: format!("q{id}"),
                conversation: None,
                surface: "cli".into(),
            },
        )
        .unwrap()
    }

    fn module(name: &str) -> NewEvent {
        NewEvent::new(
            format!("module:{name}"),
            Actor::System,
            &ModuleStarted {
                module: name.into(),
                version: "1".into(),
                pid: None,
            },
        )
        .unwrap()
    }

    /// The next `n` events' seqs, failing instead of hanging.
    async fn take_seqs<S>(stream: &mut S, n: usize) -> Vec<i64>
    where
        S: Stream<Item = Result<EventRecord>> + Unpin,
    {
        let mut seqs = Vec::with_capacity(n);
        for _ in 0..n {
            let next = tokio::time::timeout(Duration::from_secs(10), stream.next())
                .await
                .expect("an event arrives")
                .expect("the stream is open")
                .unwrap();
            seqs.push(next.seq);
        }
        seqs
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn type_patterns_match_like_globs() {
        assert!(glob_match("intent.*", "intent.answered"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("module.crashed", "module.crashed"));
        assert!(glob_match("*.started", "daemon.started"));
        assert!(!glob_match("intent.*", "intents.x"));
        assert!(!glob_match("module.crashed", "module.crashed.again"));
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn subscription_filters_by_type_glob_and_stream_prefix() {
        let (_dir, store) = store();
        store
            .append_all(vec![intent(1), module("a"), intent(2), module("b")])
            .await
            .unwrap();

        let by_type = Filter {
            types: vec!["module.*".into()],
            stream_prefix: None,
        };
        let mut modules = Box::pin(store.subscribe(by_type, 0));
        assert_eq!(take_seqs(&mut modules, 2).await, [2, 4]);

        let by_stream = Filter {
            types: Vec::new(),
            stream_prefix: Some("intent:2".into()),
        };
        let mut second = Box::pin(store.subscribe(by_stream, 0));
        assert_eq!(take_seqs(&mut second, 1).await, [3]);

        // New events are filtered the same way as stored ones.
        store
            .append_all(vec![intent(3), module("c")])
            .await
            .unwrap();
        assert_eq!(take_seqs(&mut modules, 1).await, [6]);
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn subscription_resumes_from_seq_without_gaps() {
        let (_dir, store) = store();
        store
            .append_all((1..=5).map(intent).collect())
            .await
            .unwrap();

        // A consumer that handled up to seq 3 resumes after it.
        let mut resumed = Box::pin(store.subscribe(Filter::all(), 3));
        assert_eq!(take_seqs(&mut resumed, 2).await, [4, 5]);

        // Live events keep coming in order, with nothing skipped.
        for id in 6..=8 {
            store.append(intent(id)).await.unwrap();
        }
        assert_eq!(take_seqs(&mut resumed, 3).await, [6, 7, 8]);
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn lagging_subscriber_catches_up_from_db() {
        let (_dir, store) = store();
        let mut slow = Box::pin(store.subscribe(Filter::all(), 0));
        // Far more events than the subscriber's buffer, before it reads any.
        for chunk in (1..=2_000).collect::<Vec<u32>>().chunks(250) {
            store
                .append_all(chunk.iter().copied().map(intent).collect())
                .await
                .unwrap();
        }
        let seqs = take_seqs(&mut slow, 2_000).await;
        let expected: Vec<i64> = (1..=2_000).collect();
        assert_eq!(seqs, expected, "every event, once, in order");
    }

    // covers: CORE-01
    // suite: unit
    #[tokio::test]
    async fn subscription_ends_when_the_store_is_dropped() {
        let (_dir, store) = store();
        store.append(intent(1)).await.unwrap();
        let mut stream = Box::pin(store.subscribe(Filter::all(), 0));
        assert_eq!(take_seqs(&mut stream, 1).await, [1]);
        drop(store);
        let end = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("the stream ends instead of waiting forever");
        assert!(end.is_none());
    }
}
