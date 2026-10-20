//! Projections: read models built from events (02-core §1.2).
//!
//! A projection owns its tables. The store applies every registered
//! projection to each event inside the append transaction, so a projection
//! never lags behind the log or sees an event that later rolled back. It is
//! given the full payload, even when the stored row only links to a blob.
//!
//! When the store opens, a projection that is new or whose `VERSION` changed
//! is dropped and rebuilt by replaying the whole log, and one that missed
//! events (say it wasn't registered for a while) replays just those. The
//! tables of a projection that is no longer registered are left alone.
//!
//! A projection must not block the log: an event it can't decode is skipped
//! with a warning. Only database errors fail the append.
//!
//! Phase 0 has two: `intents` (questions and answers, for follow-ups in 0.12)
//! and `modules` (each module's state, restarts and last error).

use priya_api::events::{
    EventPayload, IntentAnswered, IntentCancelled, IntentSubmitted, ModuleCrashed, ModuleStarted,
    ModuleStopped,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tracing::{info, warn};

use crate::blobs::BlobStore;
use crate::chain::Row;
use crate::error::Result;
use crate::store::EventRecord;

/// A read model kept in step with the log.
pub trait Projection: Send + Sync {
    /// The name in `projection_state`; also a prefix for its tables.
    fn name(&self) -> &'static str;
    /// Bump to rebuild the tables from the log.
    fn version(&self) -> i64;
    /// Creates the tables (they don't exist yet).
    fn create(&self, conn: &Connection) -> Result<()>;
    /// Drops the tables.
    fn drop_tables(&self, conn: &Connection) -> Result<()>;
    /// Updates the tables for one event. `payload` is the full payload,
    /// even when `record` stores it as a blob link. Events the projection
    /// doesn't care about, or can't decode, are ignored.
    fn apply(&self, conn: &Connection, record: &EventRecord, payload: &Value) -> Result<()>;
}

/// The Phase 0 projections, in the order they are applied.
pub fn phase0() -> Vec<Box<dyn Projection>> {
    vec![Box::new(Intents), Box::new(Modules)]
}

/// Brings each projection up to date when the store opens.
pub(crate) fn prepare(
    conn: &mut Connection,
    projections: &[Box<dyn Projection>],
    blobs: &dyn BlobStore,
) -> Result<()> {
    for projection in projections {
        let state: Option<(i64, i64)> = conn
            .query_row(
                "SELECT version, last_seq FROM projection_state WHERE name = ?1",
                [projection.name()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match state {
            Some((version, last_seq)) if version == projection.version() => {
                replay(conn, projection.as_ref(), blobs, last_seq, false)?;
            }
            _ => rebuild(conn, projection.as_ref(), blobs)?,
        }
    }
    Ok(())
}

/// Drops a projection's tables and replays the log into them.
pub(crate) fn rebuild(
    conn: &mut Connection,
    projection: &dyn Projection,
    blobs: &dyn BlobStore,
) -> Result<()> {
    replay(conn, projection, blobs, 0, true)
}

/// Applies the events after `after` in one transaction; with `fresh`, first
/// recreates the tables. Records how far the projection got.
fn replay(
    conn: &mut Connection,
    projection: &dyn Projection,
    blobs: &dyn BlobStore,
    after: i64,
    fresh: bool,
) -> Result<()> {
    let tx = conn.transaction()?;
    if fresh {
        projection.drop_tables(&tx)?;
        projection.create(&tx)?;
    }
    let pending: i64 =
        tx.query_row("SELECT count(*) FROM events WHERE seq > ?1", [after], |r| {
            r.get(0)
        })?;
    if pending > 0 || fresh {
        info!(
            projection = projection.name(),
            events = pending,
            fresh,
            "catching up a projection"
        );
    }
    let mut last_seq = after;
    {
        let mut statement = tx.prepare(&format!(
            "SELECT {} FROM events WHERE seq > ?1 ORDER BY seq",
            Row::COLUMNS
        ))?;
        let mut rows = statement.query([after])?;
        let mut done = 0_i64;
        while let Some(row) = rows.next()? {
            let record = EventRecord::from_row(Row::from_sql(row)?)?;
            // A lost or damaged blob must not stop the store opening: the
            // projection gets the stored link instead and skips the event.
            let payload = record.full_payload(blobs).unwrap_or_else(|e| {
                warn!(seq = record.seq, error = %e, "can't read an event's blob; replaying without it");
                record.event.payload.clone()
            });
            projection.apply(&tx, &record, &payload)?;
            last_seq = record.seq;
            done += 1;
            if done % 10_000 == 0 {
                info!(projection = projection.name(), done, pending, "catching up");
            }
        }
    }
    let head: i64 = tx.query_row("SELECT coalesce(max(seq), 0) FROM events", [], |r| r.get(0))?;
    record_state(&tx, projection, last_seq.max(head))?;
    tx.commit()?;
    Ok(())
}

/// Notes in `projection_state` how far a projection has got.
pub(crate) fn record_state(
    conn: &Connection,
    projection: &dyn Projection,
    last_seq: i64,
) -> Result<()> {
    conn.prepare_cached(
        "INSERT INTO projection_state (name, version, last_seq) VALUES (?1, ?2, ?3)
         ON CONFLICT(name) DO UPDATE SET version = excluded.version, last_seq = excluded.last_seq",
    )?
    .execute(params![projection.name(), projection.version(), last_seq])?;
    Ok(())
}

/// The payload as `P`, if the event is of `P`'s type and decodes; an event
/// that doesn't decode is skipped with a warning rather than failing the
/// append.
fn payload_of<P: EventPayload + DeserializeOwned>(
    record: &EventRecord,
    payload: &Value,
) -> Option<P> {
    if record.event.event_type != P::TYPE {
        return None;
    }
    match serde_json::from_value(payload.clone()) {
        Ok(typed) => Some(typed),
        Err(e) => {
            warn!(seq = record.seq, event_type = P::TYPE, error = %e, "projection skipped an event it can't read");
            None
        }
    }
}

/// The part after `intent:` in an intent's stream.
fn intent_id(record: &EventRecord) -> &str {
    record
        .event
        .stream
        .strip_prefix("intent:")
        .unwrap_or(&record.event.stream)
}

/// Questions asked and how they were answered. An answer wins over a
/// cancellation: a cancel after the answer changes nothing, and an answer
/// that arrives after a cancel is still recorded.
pub struct Intents;

impl Projection for Intents {
    fn name(&self) -> &'static str {
        "intents"
    }

    fn version(&self) -> i64 {
        1
    }

    fn create(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE intents (
               id            TEXT PRIMARY KEY,   -- from the stream 'intent:<id>'
               conversation  TEXT,
               text          TEXT NOT NULL,
               surface       TEXT NOT NULL,
               answer        TEXT,               -- the answer, if it is short
               answer_blob   TEXT,               -- or the blob holding the whole IntentAnswered JSON
               model         TEXT,
               status        TEXT NOT NULL CHECK (status IN ('submitted', 'answered', 'cancelled')),
               submitted_at  TEXT NOT NULL,
               finished_at   TEXT
             ) STRICT;
             CREATE INDEX intents_conversation ON intents(conversation, submitted_at);",
        )?;
        Ok(())
    }

    fn drop_tables(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch("DROP TABLE IF EXISTS intents")?;
        Ok(())
    }

    fn apply(&self, conn: &Connection, record: &EventRecord, payload: &Value) -> Result<()> {
        let id = intent_id(record);
        let ts = rfc3339(record)?;
        if let Some(submitted) = payload_of::<IntentSubmitted>(record, payload) {
            conn.prepare_cached(
                "INSERT INTO intents (id, conversation, text, surface, status, submitted_at)
                 VALUES (?1, ?2, ?3, ?4, 'submitted', ?5)
                 ON CONFLICT(id) DO NOTHING",
            )?
            .execute(params![
                id,
                submitted.conversation,
                submitted.text,
                submitted.surface,
                ts
            ])?;
        } else if let Some(answered) = payload_of::<IntentAnswered>(record, payload) {
            // A long answer stays in its blob; the table keeps the link.
            let (answer, blob) = match record.blob() {
                Some(hash) => (None, Some(hash.to_owned())),
                None => (Some(answered.answer), None),
            };
            conn.prepare_cached(
                "UPDATE intents SET answer = ?2, answer_blob = ?3, model = ?4,
                   status = 'answered', finished_at = ?5 WHERE id = ?1",
            )?
            .execute(params![id, answer, blob, answered.model, ts])?;
        } else if payload_of::<IntentCancelled>(record, payload).is_some() {
            conn.prepare_cached(
                "UPDATE intents SET status = 'cancelled', finished_at = ?2
                 WHERE id = ?1 AND status = 'submitted'",
            )?
            .execute(params![id, ts])?;
        }
        Ok(())
    }
}

/// Each module's current state.
pub struct Modules;

impl Projection for Modules {
    fn name(&self) -> &'static str {
        "modules"
    }

    fn version(&self) -> i64 {
        1
    }

    fn create(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE modules (
               name        TEXT PRIMARY KEY,
               version     TEXT,
               state       TEXT NOT NULL CHECK (state IN ('running', 'stopped', 'crashed')),
               restarts    INTEGER NOT NULL DEFAULT 0,
               last_error  TEXT,
               updated_at  TEXT NOT NULL
             ) STRICT;",
        )?;
        Ok(())
    }

    fn drop_tables(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch("DROP TABLE IF EXISTS modules")?;
        Ok(())
    }

    fn apply(&self, conn: &Connection, record: &EventRecord, payload: &Value) -> Result<()> {
        let ts = rfc3339(record)?;
        let upsert = "INSERT INTO modules (name, version, state, restarts, last_error, updated_at)
                      VALUES (?1, ?2, ?3, coalesce(?4, 0), ?5, ?6)
                      ON CONFLICT(name) DO UPDATE SET
                        version = coalesce(excluded.version, version),
                        state = excluded.state,
                        restarts = coalesce(?4, restarts),
                        last_error = coalesce(excluded.last_error, last_error),
                        updated_at = excluded.updated_at";
        if let Some(started) = payload_of::<ModuleStarted>(record, payload) {
            conn.prepare_cached(upsert)?.execute(params![
                started.module,
                started.version,
                "running",
                None::<i64>,
                None::<String>,
                ts
            ])?;
        } else if let Some(stopped) = payload_of::<ModuleStopped>(record, payload) {
            conn.prepare_cached(upsert)?.execute(params![
                stopped.module,
                None::<String>,
                "stopped",
                None::<i64>,
                None::<String>,
                ts
            ])?;
        } else if let Some(crashed) = payload_of::<ModuleCrashed>(record, payload) {
            let error =
                crashed
                    .error
                    .unwrap_or_else(|| match (crashed.exit_code, crashed.signal) {
                        (_, Some(signal)) => format!("killed by signal {signal}"),
                        (Some(code), None) => format!("exited with code {code}"),
                        (None, None) => "exited unexpectedly".into(),
                    });
            conn.prepare_cached(upsert)?.execute(params![
                crashed.module,
                None::<String>,
                "crashed",
                i64::from(crashed.restarts),
                error,
                ts
            ])?;
        }
        Ok(())
    }
}

fn rfc3339(record: &EventRecord) -> Result<String> {
    Ok(record
        .event
        .ts
        .format(&time::format_description::well_known::Rfc3339)?)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use priya_api::events::{Actor, NewEvent};
    use priya_testkit::FakeClock;
    use proptest::prelude::*;
    use proptest::test_runner::{Config as ProptestConfig, TestRunner};
    use serde_json::json;

    use super::*;
    use crate::blobs::{BlobRef, LocalBlobStore};
    use crate::db::migrate;
    use crate::store::{BLOB_KEY, EventStore, StoreConfig};

    /// One step of a random history.
    #[derive(Debug, Clone)]
    enum Step {
        Submit(u8),
        Answer(u8, bool),
        Cancel(u8),
        Start(u8),
        Stop(u8),
        Crash(u8, u32),
    }

    fn step() -> impl Strategy<Value = Step> {
        prop_oneof![
            (0..4_u8).prop_map(Step::Submit),
            (0..4_u8, any::<bool>()).prop_map(|(i, long)| Step::Answer(i, long)),
            (0..4_u8).prop_map(Step::Cancel),
            (0..3_u8).prop_map(Step::Start),
            (0..3_u8).prop_map(Step::Stop),
            (0..3_u8, 0..5_u32).prop_map(|(i, n)| Step::Crash(i, n)),
        ]
    }

    /// The step's type, stream and full payload.
    fn event(seq: i64, step: &Step) -> (&'static str, String, Value) {
        match step {
            Step::Submit(i) => (
                "intent.submitted",
                format!("intent:{i}"),
                json!({"text": format!("question {i}"), "conversation": "c1", "surface": "bar"}),
            ),
            Step::Answer(i, long) => {
                let answer = if *long {
                    format!("answer {seq} ").repeat(1_000)
                } else {
                    format!("answer {seq}")
                };
                (
                    "intent.answered",
                    format!("intent:{i}"),
                    json!({"answer": answer, "model": "m"}),
                )
            }
            Step::Cancel(i) => (
                "intent.cancelled",
                format!("intent:{i}"),
                json!({"reason": null}),
            ),
            Step::Start(i) => (
                "module.started",
                format!("module:m{i}"),
                json!({"module": format!("m{i}"), "version": "1.0", "pid": null}),
            ),
            Step::Stop(i) => (
                "module.stopped",
                format!("module:m{i}"),
                json!({"module": format!("m{i}"), "reason": "shutdown"}),
            ),
            Step::Crash(i, n) => (
                "module.crashed",
                format!("module:m{i}"),
                json!({"module": format!("m{i}"), "exit_code": 1, "signal": null, "restarts": n, "error": null}),
            ),
        }
    }

    /// Stores the step as the writer would: a long payload goes to a blob
    /// and the row links to it. The hashes are stand-ins (projections don't
    /// look at them). Returns the record and its full payload.
    fn store_step(
        conn: &Connection,
        blobs: &dyn BlobStore,
        seq: i64,
        step: &Step,
    ) -> (EventRecord, Value) {
        let (event_type, stream, payload) = event(seq, step);
        let canonical = serde_jcs::to_string(&payload).unwrap();
        let stored = if canonical.len() > crate::store::BLOB_THRESHOLD {
            let blob = blobs.put(conn, canonical.as_bytes()).unwrap();
            json!({BLOB_KEY: blob.hash, "size": blob.size}).to_string()
        } else {
            canonical
        };
        let row = Row {
            seq,
            id: ulid::Ulid::from_parts(seq as u64, 0).to_string(),
            ts: format!("2026-10-20T00:00:{:02}Z", seq % 60),
            event_type: event_type.into(),
            version: 1,
            stream,
            correlation: None,
            causation: None,
            actor: "system".into(),
            payload: stored,
            prev_hash: "0".repeat(64),
            hash: "0".repeat(64),
        };
        row.insert(conn).unwrap();
        (EventRecord::from_row(row).unwrap(), payload)
    }

    /// Blobs kept in memory: the property test stores thousands, and the
    /// real store's fsyncs would make it slow. Only `put` and `get` are used.
    #[derive(Default)]
    struct MemoryBlobs(Mutex<HashMap<String, Vec<u8>>>);

    impl BlobStore for MemoryBlobs {
        fn put(&self, _: &Connection, data: &[u8]) -> Result<BlobRef> {
            let hash = blake3::hash(data).to_hex().to_string();
            self.0.lock().unwrap().insert(hash.clone(), data.to_vec());
            Ok(BlobRef {
                hash,
                size: data.len() as u64,
            })
        }
        fn get(&self, hash: &str) -> Result<Vec<u8>> {
            Ok(self.0.lock().unwrap()[hash].clone())
        }
        fn incref(&self, _: &Connection, _: &str) -> Result<()> {
            unreachable!("not used by projections")
        }
        fn decref(&self, _: &Connection, _: &str) -> Result<()> {
            unreachable!("not used by projections")
        }
        fn gc(&self, _: &Connection) -> Result<usize> {
            unreachable!("not used by projections")
        }
    }

    fn database() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, Path::new(":memory:")).unwrap();
        conn
    }

    /// Every row of both Phase 0 tables, as text.
    fn dump(conn: &Connection) -> Vec<String> {
        let mut out = Vec::new();
        for table in ["intents", "modules"] {
            let mut statement = conn
                .prepare(&format!("SELECT * FROM {table} ORDER BY 1"))
                .unwrap();
            let columns = statement.column_count();
            let rows = statement
                .query_map([], |r| {
                    (0..columns)
                        .map(|i| {
                            r.get::<_, rusqlite::types::Value>(i)
                                .map(|v| format!("{v:?}"))
                        })
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap();
            for row in rows {
                out.push(format!("{table}: {}", row.unwrap().join(" | ")));
            }
        }
        out
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[test]
    fn projection_rebuild_equals_incremental() {
        let blobs = MemoryBlobs::default();
        let mut runner = TestRunner::new(ProptestConfig::with_cases(1000));
        let histories = prop::collection::vec(step(), 0..40);
        runner
            .run(&histories, |steps| {
                let projections = phase0();

                // Applied one event at a time with the full payload, as
                // appends do.
                let mut incremental = database();
                prepare(&mut incremental, &projections, &blobs).unwrap();
                for (i, step) in steps.iter().enumerate() {
                    let (record, payload) = store_step(&incremental, &blobs, i as i64 + 1, step);
                    for projection in &projections {
                        projection.apply(&incremental, &record, &payload).unwrap();
                    }
                }

                // Rebuilt from the stored log in one go, loading blobs.
                let mut rebuilt = database();
                for (i, step) in steps.iter().enumerate() {
                    store_step(&rebuilt, &blobs, i as i64 + 1, step);
                }
                for projection in &projections {
                    rebuild(&mut rebuilt, projection.as_ref(), &blobs).unwrap();
                }

                prop_assert_eq!(dump(&incremental), dump(&rebuilt));
                Ok(())
            })
            .unwrap();
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn events_a_projection_cannot_read_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let blobs = LocalBlobStore::new(dir.path(), Arc::new(FakeClock::new()));
        let mut conn = database();
        prepare(&mut conn, &phase0(), &blobs).unwrap();
        let (record, _) = store_step(&conn, &blobs, 1, &Step::Submit(1));
        // A payload that doesn't fit the type (only a bug could store one).
        let unreadable = json!({"text": 5});
        for projection in phase0() {
            projection.apply(&conn, &record, &unreadable).unwrap();
        }
        assert!(dump(&conn).is_empty(), "skipped, and the append goes on");
    }

    /// Counts events of one type; its version decides which type.
    struct Counter {
        version: i64,
    }

    impl Projection for Counter {
        fn name(&self) -> &'static str {
            "counter"
        }
        fn version(&self) -> i64 {
            self.version
        }
        fn create(&self, conn: &Connection) -> Result<()> {
            conn.execute_batch(
                "CREATE TABLE counter (n INTEGER NOT NULL); INSERT INTO counter VALUES (0);",
            )?;
            Ok(())
        }
        fn drop_tables(&self, conn: &Connection) -> Result<()> {
            conn.execute_batch("DROP TABLE IF EXISTS counter")?;
            Ok(())
        }
        fn apply(&self, conn: &Connection, record: &EventRecord, _: &Value) -> Result<()> {
            let counted = if self.version == 1 {
                "module.started"
            } else {
                "intent.submitted"
            };
            if record.event.event_type == counted {
                conn.execute_batch("UPDATE counter SET n = n + 1")?;
            }
            Ok(())
        }
    }

    fn submitted(id: u32) -> NewEvent {
        NewEvent::new(
            format!("intent:{id}"),
            Actor::User,
            &IntentSubmitted {
                text: format!("question {id}"),
                conversation: None,
                surface: "cli".into(),
            },
        )
        .unwrap()
    }

    fn started(name: &str) -> NewEvent {
        NewEvent::new(
            format!("module:{name}"),
            Actor::System,
            &ModuleStarted {
                module: name.into(),
                version: "1.0".into(),
                pid: Some(42),
            },
        )
        .unwrap()
    }

    fn answer(id: u32, text: String) -> NewEvent {
        NewEvent::new(
            format!("intent:{id}"),
            Actor::PriyaAi,
            &IntentAnswered {
                answer: text,
                model: "claude".into(),
            },
        )
        .unwrap()
    }

    fn open(config: &StoreConfig, projections: Vec<Box<dyn Projection>>) -> EventStore {
        EventStore::open_with(config, Arc::new(FakeClock::new()), projections).unwrap()
    }

    fn counter(version: i64) -> Vec<Box<dyn Projection>> {
        vec![Box::new(Counter { version })]
    }

    async fn count(store: &EventStore) -> (i64, i64) {
        store
            .query(|conn| {
                let n = conn.query_row("SELECT n FROM counter", [], |r| r.get(0))?;
                let version = conn.query_row(
                    "SELECT version FROM projection_state WHERE name = 'counter'",
                    [],
                    |r| r.get(0),
                )?;
                Ok((n, version))
            })
            .await
            .unwrap()
    }

    async fn phase0_tables(store: &EventStore) -> Vec<String> {
        store.query(|conn| Ok(dump(conn))).await.unwrap()
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn projection_version_bump_triggers_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let config = StoreConfig::in_data_dir(dir.path());
        {
            let store = open(&config, counter(1));
            store
                .append_all(vec![
                    started("a"),
                    submitted(1),
                    submitted(2),
                    started("b"),
                    started("c"),
                ])
                .await
                .unwrap();
            assert_eq!(count(&store).await, (3, 1), "v1 counts module starts");
        }
        let store = open(&config, counter(1));
        assert_eq!(
            count(&store).await,
            (3, 1),
            "same version: kept, not rebuilt"
        );
        drop(store);

        let store = open(&config, counter(2));
        assert_eq!(count(&store).await, (2, 2), "v2 rebuilt from the log");
        store.append(submitted(3)).await.unwrap();
        assert_eq!(
            count(&store).await,
            (3, 2),
            "and kept up to date afterwards"
        );
        store.rebuild_projection("counter").await.unwrap();
        assert_eq!(
            count(&store).await,
            (3, 2),
            "a rebuild on request gives the same"
        );
        assert!(store.rebuild_projection("nope").await.is_err());
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn a_projection_that_missed_events_catches_up_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let config = StoreConfig::in_data_dir(dir.path());
        {
            let store = open(&config, counter(1));
            store.append(started("a")).await.unwrap();
        }
        {
            // Opened without the counter: it misses these.
            let store = open(&config, Vec::new());
            store
                .append_all(vec![started("b"), submitted(1), started("c")])
                .await
                .unwrap();
        }
        let store = open(&config, counter(1));
        assert_eq!(count(&store).await, (3, 1), "caught up, not rebuilt");
        let last_seq: i64 = store
            .query(|conn| {
                Ok(conn.query_row(
                    "SELECT last_seq FROM projection_state WHERE name = 'counter'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .await
            .unwrap();
        assert_eq!(last_seq, 4);
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn large_payloads_reach_the_projections() {
        let dir = tempfile::tempdir().unwrap();
        let config = StoreConfig::in_data_dir(dir.path());
        let store = open(&config, phase0());
        let long_question = NewEvent::new(
            "intent:1",
            Actor::User,
            &IntentSubmitted {
                text: "why ".repeat(5_000),
                conversation: Some("c1".into()),
                surface: "bar".into(),
            },
        )
        .unwrap();
        let long_crash = NewEvent::new(
            "module:weather",
            Actor::System,
            &ModuleCrashed {
                module: "weather".into(),
                exit_code: Some(101),
                signal: None,
                restarts: 2,
                error: Some("panicked: ".repeat(2_000)),
            },
        )
        .unwrap();
        let records = store
            .append_all(vec![
                long_question,
                started("weather"),
                long_crash,
                answer(1, "because ".repeat(2_000)),
            ])
            .await
            .unwrap();
        assert!(
            [0, 2, 3].iter().all(|&i| records[i].blob().is_some()),
            "each large payload went to a blob"
        );

        let appended = phase0_tables(&store).await;
        assert_eq!(appended.len(), 2, "{appended:?}");
        assert!(appended[0].contains("why why"), "the question is there");
        assert!(appended[0].contains("\"claude\""), "the model is kept");
        assert!(appended[1].contains("crashed") && appended[1].contains("panicked"));

        for name in ["intents", "modules"] {
            store.rebuild_projection(name).await.unwrap();
        }
        assert_eq!(phase0_tables(&store).await, appended, "a rebuild agrees");
    }

    // covers: CORE-01
    // suite: unit
    #[tokio::test]
    async fn a_lost_blob_does_not_stop_a_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let config = StoreConfig::in_data_dir(dir.path());
        let hash = {
            let store = open(&config, phase0());
            store.append(submitted(1)).await.unwrap();
            let long = store
                .append(answer(1, "long ".repeat(5_000)))
                .await
                .unwrap();
            long.blob().unwrap().to_owned()
        };
        std::fs::remove_file(config.blobs.join(&hash[..2]).join(&hash)).unwrap();

        // A version bump or a rebuild on request replays the lost event.
        let store = open(&config, phase0());
        store.rebuild_projection("intents").await.unwrap();
        let tables = phase0_tables(&store).await;
        assert_eq!(tables.len(), 1);
        assert!(
            tables[0].contains("submitted"),
            "the answer is skipped: {tables:?}"
        );
        store.append(submitted(2)).await.unwrap();
    }

    // covers: CORE-01
    // suite: unit
    #[tokio::test]
    async fn intents_follow_questions_answers_and_cancellations() {
        /// id, status, answer, answer blob, model
        type IntentRow = (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let dir = tempfile::tempdir().unwrap();
        let store = open(&StoreConfig::in_data_dir(dir.path()), phase0());
        let cancel = |id: u32| {
            NewEvent::new(
                format!("intent:{id}"),
                Actor::User,
                &IntentCancelled { reason: None },
            )
            .unwrap()
        };
        store
            .append_all(vec![submitted(1), submitted(2), submitted(3), submitted(4)])
            .await
            .unwrap();
        store.append(answer(1, "short".into())).await.unwrap();
        let long = store
            .append(answer(2, "long ".repeat(5_000)))
            .await
            .unwrap();
        store.append(cancel(3)).await.unwrap();
        store.append(cancel(1)).await.unwrap(); // too late: already answered
        store.append(cancel(4)).await.unwrap();
        store.append(answer(4, "late".into())).await.unwrap(); // still recorded

        let rows: Vec<IntentRow> = store
            .query(|conn| {
                let mut statement = conn.prepare(
                    "SELECT id, status, answer, answer_blob, model FROM intents ORDER BY id",
                )?;
                let rows = statement
                    .query_map([], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await
            .unwrap();
        let blob = long.blob().map(str::to_owned);
        let claude = Some("claude".to_owned());
        assert_eq!(
            rows,
            [
                (
                    "1".into(),
                    "answered".into(),
                    Some("short".into()),
                    None,
                    claude.clone()
                ),
                ("2".into(), "answered".into(), None, blob, claude.clone()),
                ("3".into(), "cancelled".into(), None, None, None),
                (
                    "4".into(),
                    "answered".into(),
                    Some("late".into()),
                    None,
                    claude
                ),
            ]
        );
    }
}
