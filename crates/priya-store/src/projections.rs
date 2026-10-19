//! Projections: read models built from events (02-core §1.2).
//!
//! A projection owns its tables. The store applies every registered
//! projection to each event inside the append transaction, so a projection
//! never lags behind the log or sees an event that later rolled back. When a
//! projection's `VERSION` changes, or on request, its tables are dropped and
//! rebuilt by replaying the whole log, with progress logged.
//!
//! Phase 0 has two: `intents` (questions and answers, for follow-ups in 0.12)
//! and `modules` (each module's state, restarts and last error).

use priya_api::events::{
    EventPayload, IntentAnswered, IntentCancelled, IntentSubmitted, ModuleCrashed, ModuleStarted,
    ModuleStopped,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;
use tracing::info;

use crate::chain::Row;
use crate::error::{Result, StoreError};
use crate::store::{BLOB_KEY, EventRecord};

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
    /// Updates the tables for one event. Events the projection doesn't care
    /// about are ignored.
    fn apply(&self, conn: &Connection, record: &EventRecord) -> Result<()>;
}

/// The Phase 0 projections, in the order they are applied.
pub fn phase0() -> Vec<Box<dyn Projection>> {
    vec![Box::new(Intents), Box::new(Modules)]
}

/// Brings each projection up to date when the store opens: a new or
/// re-versioned projection is rebuilt from the whole log.
pub(crate) fn prepare(conn: &mut Connection, projections: &[Box<dyn Projection>]) -> Result<()> {
    for projection in projections {
        let state: Option<i64> = conn
            .query_row(
                "SELECT version FROM projection_state WHERE name = ?1",
                [projection.name()],
                |r| r.get(0),
            )
            .optional()?;
        if state != Some(projection.version()) {
            rebuild(conn, projection.as_ref())?;
        }
    }
    Ok(())
}

/// Drops a projection's tables and replays the log into them, in one
/// transaction.
pub(crate) fn rebuild(conn: &mut Connection, projection: &dyn Projection) -> Result<()> {
    let tx = conn.transaction()?;
    projection.drop_tables(&tx)?;
    projection.create(&tx)?;
    let total: i64 = tx.query_row("SELECT count(*) FROM events", [], |r| r.get(0))?;
    info!(
        projection = projection.name(),
        events = total,
        "rebuilding a projection"
    );
    let mut last_seq = 0;
    {
        let mut statement =
            tx.prepare(&format!("SELECT {} FROM events ORDER BY seq", Row::COLUMNS))?;
        let mut rows = statement.query([])?;
        let mut done = 0_i64;
        while let Some(row) = rows.next()? {
            let record = EventRecord::from_row(Row::from_sql(row)?)?;
            projection.apply(&tx, &record)?;
            last_seq = record.seq;
            done += 1;
            if done % 10_000 == 0 {
                info!(projection = projection.name(), done, total, "rebuilding");
            }
        }
    }
    record_state(&tx, projection, last_seq)?;
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

/// The payload as `P`, if the event is of `P`'s type.
fn payload_of<P: EventPayload + DeserializeOwned>(record: &EventRecord) -> Result<Option<P>> {
    if record.event.event_type != P::TYPE {
        return Ok(None);
    }
    serde_json::from_value(record.event.payload.clone())
        .map(Some)
        .map_err(|e| StoreError::Corrupt(format!("event {}: {e}", record.seq)))
}

/// The part after `intent:` in an intent's stream.
fn intent_id(record: &EventRecord) -> &str {
    record
        .event
        .stream
        .strip_prefix("intent:")
        .unwrap_or(&record.event.stream)
}

/// Questions asked and how they were answered.
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
               answer        TEXT,               -- inline answer
               answer_blob   TEXT,               -- or the blob holding it
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

    fn apply(&self, conn: &Connection, record: &EventRecord) -> Result<()> {
        let id = intent_id(record);
        let ts = rfc3339(record)?;
        if let Some(submitted) = payload_of::<IntentSubmitted>(record)? {
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
        } else if record.event.event_type == IntentAnswered::TYPE {
            // A long answer stays in its blob; the projection keeps the link.
            let (answer, blob, model) = match record.event.payload.get(BLOB_KEY) {
                Some(hash) => (None, hash.as_str().map(str::to_owned), None),
                None => {
                    let answered = payload_of::<IntentAnswered>(record)?;
                    let answered = answered.ok_or_else(|| {
                        StoreError::Corrupt(format!("event {}: not an answer", record.seq))
                    })?;
                    (Some(answered.answer), None, Some(answered.model))
                }
            };
            conn.prepare_cached(
                "UPDATE intents SET answer = ?2, answer_blob = ?3, model = coalesce(?4, model),
                   status = 'answered', finished_at = ?5 WHERE id = ?1",
            )?
            .execute(params![id, answer, blob, model, ts])?;
        } else if payload_of::<IntentCancelled>(record)?.is_some() {
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

    fn apply(&self, conn: &Connection, record: &EventRecord) -> Result<()> {
        let ts = rfc3339(record)?;
        let upsert = "INSERT INTO modules (name, version, state, restarts, last_error, updated_at)
                      VALUES (?1, ?2, ?3, coalesce(?4, 0), ?5, ?6)
                      ON CONFLICT(name) DO UPDATE SET
                        version = coalesce(excluded.version, version),
                        state = excluded.state,
                        restarts = coalesce(?4, restarts),
                        last_error = coalesce(excluded.last_error, last_error),
                        updated_at = excluded.updated_at";
        if let Some(started) = payload_of::<ModuleStarted>(record)? {
            conn.prepare_cached(upsert)?.execute(params![
                started.module,
                started.version,
                "running",
                None::<i64>,
                None::<String>,
                ts
            ])?;
        } else if let Some(stopped) = payload_of::<ModuleStopped>(record)? {
            conn.prepare_cached(upsert)?.execute(params![
                stopped.module,
                None::<String>,
                "stopped",
                None::<i64>,
                None::<String>,
                ts
            ])?;
        } else if let Some(crashed) = payload_of::<ModuleCrashed>(record)? {
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
    use std::path::Path;
    use std::sync::Arc;

    use priya_api::events::{Actor, NewEvent};
    use priya_testkit::FakeClock;
    use proptest::prelude::*;
    use proptest::test_runner::{Config as ProptestConfig, TestRunner};
    use serde_json::json;

    use super::*;
    use crate::db::migrate;
    use crate::store::{EventStore, StoreConfig};

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

    /// The step as a stored row, with a stand-in hash (projections don't
    /// look at hashes).
    fn row(seq: i64, step: &Step) -> Row {
        let (event_type, stream, payload) = match step {
            Step::Submit(i) => (
                "intent.submitted",
                format!("intent:{i}"),
                json!({"text": format!("question {i}"), "conversation": "c1", "surface": "bar"}),
            ),
            // A long answer is stored as a blob reference.
            Step::Answer(i, true) => (
                "intent.answered",
                format!("intent:{i}"),
                json!({"$blob": format!("{:064x}", seq), "size": 20_000}),
            ),
            Step::Answer(i, false) => (
                "intent.answered",
                format!("intent:{i}"),
                json!({"answer": format!("answer {seq}"), "model": "m"}),
            ),
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
        };
        Row {
            seq,
            id: ulid::Ulid::from_parts(seq as u64, 0).to_string(),
            ts: format!("2026-10-20T00:00:{:02}Z", seq % 60),
            event_type: event_type.into(),
            version: 1,
            stream,
            correlation: None,
            causation: None,
            actor: "system".into(),
            payload: payload.to_string(),
            prev_hash: "0".repeat(64),
            hash: "0".repeat(64),
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
    #[test]
    fn projection_rebuild_equals_incremental() {
        let mut runner = TestRunner::new(ProptestConfig::with_cases(1000));
        let histories = prop::collection::vec(step(), 0..40);
        runner
            .run(&histories, |steps| {
                let projections = phase0();
                let rows: Vec<Row> = steps
                    .iter()
                    .enumerate()
                    .map(|(i, s)| row(i as i64 + 1, s))
                    .collect();

                // Applied one event at a time, as appends do.
                let mut incremental = database();
                prepare(&mut incremental, &projections).unwrap();
                for r in &rows {
                    r.insert(&incremental).unwrap();
                    let record = EventRecord::from_row(r.clone()).unwrap();
                    for projection in &projections {
                        projection.apply(&incremental, &record).unwrap();
                    }
                }

                // Rebuilt from the stored log in one go.
                let mut rebuilt = database();
                for r in &rows {
                    r.insert(&rebuilt).unwrap();
                }
                for projection in &projections {
                    rebuild(&mut rebuilt, projection.as_ref()).unwrap();
                }

                prop_assert_eq!(dump(&incremental), dump(&rebuilt));
                Ok(())
            })
            .unwrap();
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
        fn apply(&self, conn: &Connection, record: &EventRecord) -> Result<()> {
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

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    #[tokio::test]
    async fn projection_version_bump_triggers_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let config = StoreConfig::in_data_dir(dir.path());
        let open = |version| {
            EventStore::open_with(
                &config,
                Arc::new(FakeClock::new()),
                vec![Box::new(Counter { version }) as Box<dyn Projection>],
            )
            .unwrap()
        };
        {
            let store = open(1);
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
        let store = open(1);
        assert_eq!(
            count(&store).await,
            (3, 1),
            "same version: kept, not rebuilt"
        );
        drop(store);

        let store = open(2);
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
        let store = EventStore::open(
            &StoreConfig::in_data_dir(dir.path()),
            Arc::new(FakeClock::new()),
        )
        .unwrap();
        let answer = |id: u32, text: String| {
            NewEvent::new(
                format!("intent:{id}"),
                Actor::PriyaAi,
                &IntentAnswered {
                    answer: text,
                    model: "claude".into(),
                },
            )
            .unwrap()
        };
        let cancel = |id: u32| {
            NewEvent::new(
                format!("intent:{id}"),
                Actor::User,
                &IntentCancelled { reason: None },
            )
            .unwrap()
        };
        store
            .append_all(vec![submitted(1), submitted(2), submitted(3)])
            .await
            .unwrap();
        store.append(answer(1, "short".into())).await.unwrap();
        let long = store
            .append(answer(2, "long ".repeat(5_000)))
            .await
            .unwrap();
        store.append(cancel(3)).await.unwrap();
        store.append(cancel(1)).await.unwrap(); // too late: already answered

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
        assert_eq!(
            rows,
            [
                (
                    "1".into(),
                    "answered".into(),
                    Some("short".into()),
                    None,
                    Some("claude".into())
                ),
                ("2".into(), "answered".into(), None, blob, None),
                ("3".into(), "cancelled".into(), None, None, None),
            ]
        );
    }
}
