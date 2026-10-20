//! [`EventStore`]: appending and reading events.
//!
//! One writer thread owns the only write connection and takes requests from a
//! bounded queue, so appends are serialised without locks and the chain head
//! (last seq and hash) lives in one place. Reads use a small pool of
//! read-only connections on blocking threads, so they never wait for the
//! writer (WAL lets readers and the writer work at once).
//!
//! Each append, in one transaction per batch:
//! 1. looks up the `(type, version)` and validates the payload, which comes
//!    back in canonical form;
//! 2. moves a payload over 8 KiB (as canonical JSON) to the blob store, leaving
//!    `{"$blob": "<hash>", "size": n}` in the row;
//! 3. gives the event the next seq, a ULID and the clock's time, links it to
//!    the previous hash and hashes it (see [`crate::chain`]).
//!
//! A batch is all-or-nothing: one invalid event and none are written.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::SystemTime;

use priya_api::events::{self, EventEnvelope, NewEvent};
use priya_core::clock::Clock;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::warn;
use ulid::{Generator, Ulid};

use crate::blobs::{BlobStore, LocalBlobStore};
use crate::chain::{self, ChainReport, GENESIS, Row};
use crate::db::{self, Access};
use crate::error::{Result, StoreError};
use crate::projections::{self, Projection};
use crate::subscriptions::{self, Filter};

/// Payloads bigger than this (as canonical JSON) are stored as blobs.
pub const BLOB_THRESHOLD: usize = 8 * 1024;
/// The key that marks a payload moved to the blob store.
pub const BLOB_KEY: &str = "$blob";

/// Where the store keeps things and how big its queues are.
#[derive(Debug, Clone)]
pub struct StoreConfig {
    pub db: PathBuf,
    pub blobs: PathBuf,
    /// Reads that may run at once, each on its own read-only connection.
    pub readers: usize,
    /// Append requests that can wait for the writer.
    pub queue: usize,
}

impl StoreConfig {
    /// `<data>/priya.db` and `<data>/blobs`, as in `$XDG_DATA_HOME/priya`.
    pub fn in_data_dir(data: &Path) -> Self {
        Self {
            db: data.join("priya.db"),
            blobs: data.join("blobs"),
            readers: 4,
            queue: 256,
        }
    }
}

/// A stored event: the envelope, its place in the log and its hashes. The
/// payload is as stored, so a large one is a `$blob` reference; use
/// [`EventStore::payload`] for the full payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventRecord {
    pub seq: i64,
    #[serde(flatten)]
    pub event: EventEnvelope,
    pub prev_hash: String,
    pub hash: String,
}

impl EventRecord {
    pub(crate) fn from_row(row: Row) -> Result<Self> {
        let corrupt = |what: &str, e: &dyn std::fmt::Display| {
            StoreError::Corrupt(format!("event {}: {what}: {e}", row.seq))
        };
        let event = EventEnvelope {
            id: row.id.parse().map_err(|e| corrupt("id", &e))?,
            ts: OffsetDateTime::parse(&row.ts, &Rfc3339).map_err(|e| corrupt("ts", &e))?,
            event_type: row.event_type,
            version: u16::try_from(row.version).map_err(|e| corrupt("version", &e))?,
            stream: row.stream,
            correlation: row.correlation,
            causation: row
                .causation
                .map(|c| c.parse::<Ulid>())
                .transpose()
                .map_err(|e| corrupt("causation", &e))?,
            actor: row
                .actor
                .parse()
                .map_err(|e: String| corrupt("actor", &e))?,
            payload: serde_json::from_str(&row.payload).map_err(|e| corrupt("payload", &e))?,
        };
        Ok(Self {
            seq: row.seq,
            event,
            prev_hash: row.prev_hash,
            hash: row.hash,
        })
    }

    /// The blob this event's payload was moved to, if any.
    pub fn blob(&self) -> Option<&str> {
        self.event.payload.get(BLOB_KEY).and_then(Value::as_str)
    }

    /// The payload as written, loaded from its blob if it was moved there.
    pub(crate) fn full_payload(&self, blobs: &dyn BlobStore) -> Result<Value> {
        let Some(hash) = self.blob() else {
            return Ok(self.event.payload.clone());
        };
        let bytes = blobs.get(hash)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| StoreError::Corrupt(format!("event {}: blob payload: {e}", self.seq)))
    }
}

impl From<EventRecord> for priya_api::methods::events::StoredEvent {
    /// The API's shape. The payload is as stored; resolve a blob link with
    /// [`EventStore::payload`] first to send the full payload.
    fn from(record: EventRecord) -> Self {
        Self {
            seq: record.seq,
            event: record.event,
            prev_hash: record.prev_hash,
            hash: record.hash,
        }
    }
}

/// What the writer can be asked to do.
enum Command {
    Append {
        events: Vec<NewEvent>,
        reply: oneshot::Sender<Result<Vec<EventRecord>>>,
    },
    Rebuild {
        projection: String,
        reply: oneshot::Sender<Result<()>>,
    },
}

/// The append-only event log. Cheap to clone; all clones share one writer.
#[derive(Clone)]
pub struct EventStore {
    inner: Arc<Inner>,
}

struct Inner {
    queue: Option<mpsc::Sender<Command>>,
    /// The seq of the newest committed event, for subscriptions.
    head: watch::Receiver<i64>,
    writer: Option<JoinHandle<()>>,
    reader: Arc<Reader>,
}

impl EventStore {
    /// Opens (or creates) the store with the Phase 0 projections: migrates
    /// the schema, collects unused blobs, brings projections up to date,
    /// then starts the writer.
    pub fn open(config: &StoreConfig, clock: Arc<dyn Clock>) -> Result<Self> {
        Self::open_with(config, clock, projections::phase0())
    }

    /// [`open`](Self::open) with a chosen set of projections.
    pub fn open_with(
        config: &StoreConfig,
        clock: Arc<dyn Clock>,
        projections: Vec<Box<dyn Projection>>,
    ) -> Result<Self> {
        if let Some(dir) = config.db.parent() {
            std::fs::create_dir_all(dir).map_err(StoreError::io(dir))?;
        }
        let mut conn = db::open(&config.db, Access::Write)?;
        db::migrate(&mut conn, &config.db)?;
        let blobs = Arc::new(LocalBlobStore::new(&config.blobs, clock.clone()));
        // Before the writer starts: gc mustn't race a put.
        let collected = blobs.gc(&conn)?;
        if collected > 0 {
            tracing::info!(collected, "removed unused blobs");
        }

        projections::prepare(&mut conn, &projections, blobs.as_ref())?;

        let head = Head::load(&conn)?;
        let (published, head_seq) = watch::channel(head.seq);
        let (queue, requests) = mpsc::channel(config.queue.max(1));
        let writer = Writer {
            conn,
            head,
            clock,
            blobs: blobs.clone(),
            ids: Generator::new(),
            projections,
            published,
        };
        let writer = std::thread::Builder::new()
            .name("priya-store-writer".into())
            .spawn(move || writer.run(requests))
            .map_err(StoreError::io(&config.db))?;

        Ok(Self {
            inner: Arc::new(Inner {
                queue: Some(queue),
                head: head_seq,
                writer: Some(writer),
                reader: Arc::new(Reader {
                    pool: ReadPool::new(&config.db, config.readers),
                    slots: Arc::new(tokio::sync::Semaphore::new(config.readers.max(1))),
                    blobs,
                }),
            }),
        })
    }

    /// Appends one event.
    pub async fn append(&self, event: NewEvent) -> Result<EventRecord> {
        let mut records = self.append_all(vec![event]).await?;
        records.pop().ok_or(StoreError::Closed)
    }

    /// Appends events in one transaction: all of them, or none. Every
    /// projection is updated in the same transaction.
    pub async fn append_all(&self, events: Vec<NewEvent>) -> Result<Vec<EventRecord>> {
        self.ask(|reply| Command::Append { events, reply }).await
    }

    /// Drops a projection's tables and rebuilds them from the whole log.
    pub async fn rebuild_projection(&self, name: &str) -> Result<()> {
        let projection = name.to_owned();
        self.ask(|reply| Command::Rebuild { projection, reply })
            .await
    }

    /// Events matching `filter` with seq greater than `after`: first those
    /// already stored, then new ones as they are committed. Delivery is in
    /// seq order and at least once; see [`crate::subscriptions`].
    ///
    /// The stream ends when the store is dropped, or after the first error
    /// (resubscribe after the last seq handled). Call it inside a tokio
    /// runtime: it spawns the task that feeds the stream.
    pub fn subscribe(
        &self,
        filter: Filter,
        after: i64,
    ) -> impl futures_core::Stream<Item = Result<EventRecord>> + use<> {
        subscriptions::spawn(
            self.inner.reader.clone(),
            self.inner.head.clone(),
            filter,
            after,
        )
    }

    /// Sends a command to the writer and waits for its answer.
    async fn ask<R>(
        &self,
        command: impl FnOnce(oneshot::Sender<Result<R>>) -> Command,
    ) -> Result<R> {
        let queue = self.inner.queue.as_ref().ok_or(StoreError::Closed)?;
        let (reply, answer) = oneshot::channel();
        queue
            .send(command(reply))
            .await
            .map_err(|_| StoreError::Closed)?;
        answer.await.map_err(|_| StoreError::Closed)?
    }

    /// Runs `f` on a read-only connection; for projections' own queries.
    pub async fn query<R: Send + 'static>(
        &self,
        f: impl FnOnce(&Connection) -> Result<R> + Send + 'static,
    ) -> Result<R> {
        self.inner.reader.run(f).await
    }

    /// Up to `limit` events with seq greater than `after`, in order.
    pub async fn read_after(&self, after: i64, limit: usize) -> Result<Vec<EventRecord>> {
        self.inner.reader.read_after(after, limit).await
    }

    /// Checks the hash chain over `range` (see [`chain::verify`]).
    pub async fn verify_chain(&self, range: RangeInclusive<i64>) -> Result<ChainReport> {
        self.inner
            .reader
            .run(move |conn| chain::verify(conn, range))
            .await
    }

    /// The full payload of an event, fetched from the blob store if it was
    /// moved there.
    pub async fn payload(&self, record: &EventRecord) -> Result<Value> {
        if record.blob().is_none() {
            return Ok(record.event.payload.clone());
        }
        let reader = self.inner.reader.clone();
        let record = record.clone();
        tokio::task::spawn_blocking(move || record.full_payload(reader.blobs.as_ref()))
            .await
            .map_err(|_| StoreError::Internal("a blob read panicked".into()))?
    }
}

/// The read side of the store. Subscriptions hold only this, not the
/// [`EventStore`], so dropping the last store stops the writer and ends them.
pub(crate) struct Reader {
    pool: ReadPool,
    /// At most `readers` reads at once; others wait their turn.
    slots: Arc<tokio::sync::Semaphore>,
    blobs: Arc<LocalBlobStore>,
}

impl Reader {
    /// Runs `f` on a read-only connection, on a blocking thread.
    pub(crate) async fn run<R: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&Connection) -> Result<R> + Send + 'static,
    ) -> Result<R> {
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| StoreError::Closed)?;
        let reader = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            reader.pool.with(f)
        })
        .await
        .map_err(|_| StoreError::Internal("a read panicked".into()))?
    }

    /// Up to `limit` events with seq greater than `after`, in order.
    pub(crate) async fn read_after(
        self: &Arc<Self>,
        after: i64,
        limit: usize,
    ) -> Result<Vec<EventRecord>> {
        self.run(move |conn| {
            let mut statement = conn.prepare_cached(&format!(
                "SELECT {} FROM events WHERE seq > ?1 ORDER BY seq LIMIT ?2",
                Row::COLUMNS
            ))?;
            let rows = statement
                .query_map(rusqlite::params![after, limit as i64], Row::from_sql)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows.into_iter().map(EventRecord::from_row).collect()
        })
        .await
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Closing the queue ends the writer once it has finished what it has.
        // Joining may block for that batch's fsyncs; the writer never needs
        // the async runtime, so it can't deadlock on it.
        drop(self.queue.take());
        if let Some(writer) = self.writer.take()
            && writer.join().is_err()
        {
            warn!("the store's writer thread panicked");
        }
    }
}

/// The last seq and hash: where the next event links in.
struct Head {
    seq: i64,
    hash: String,
}

impl Head {
    /// The next event links to the last row's hash, and takes a seq past
    /// any ever used: if the newest rows were deleted, AUTOINCREMENT's
    /// `sqlite_sequence` remembers them, so the next seq leaves a visible gap
    /// instead of quietly reusing theirs.
    fn load(conn: &Connection) -> Result<Self> {
        let last: Option<(i64, String)> = conn
            .query_row(
                "SELECT seq, hash FROM events ORDER BY seq DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let used: i64 = conn.query_row(
            "SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name = 'events'), 0)",
            [],
            |r| r.get(0),
        )?;
        let (seq, hash) = last.unwrap_or_else(|| (0, GENESIS.to_owned()));
        Ok(Self {
            seq: seq.max(used),
            hash,
        })
    }
}

struct Writer {
    conn: Connection,
    head: Head,
    clock: Arc<dyn Clock>,
    blobs: Arc<LocalBlobStore>,
    ids: Generator,
    projections: Vec<Box<dyn Projection>>,
    /// Told the new head seq after each commit.
    published: watch::Sender<i64>,
}

impl Writer {
    fn run(mut self, mut requests: mpsc::Receiver<Command>) {
        while let Some(command) = requests.blocking_recv() {
            // A panic is a bug: report it to this caller and keep serving the
            // rest. The unwinding transaction rolls back, and the head only
            // moves after a commit, so the state stays consistent. A caller
            // that stopped waiting doesn't matter: the work is done anyway.
            match command {
                Command::Append { events, reply } => {
                    let _ = reply.send(guard(|| self.append(events)));
                }
                Command::Rebuild { projection, reply } => {
                    let _ = reply.send(guard(|| self.rebuild(&projection)));
                }
            }
        }
    }

    fn rebuild(&mut self, name: &str) -> Result<()> {
        let projection = self
            .projections
            .iter()
            .find(|p| p.name() == name)
            .ok_or_else(|| StoreError::Internal(format!("no projection named {name}")))?;
        projections::rebuild(&mut self.conn, projection.as_ref(), self.blobs.as_ref())
    }

    fn append(&mut self, events: Vec<NewEvent>) -> Result<Vec<EventRecord>> {
        let tx = self.conn.transaction()?;
        let mut head = Head {
            seq: self.head.seq,
            hash: self.head.hash.clone(),
        };
        let mut records = Vec::with_capacity(events.len());
        for event in events {
            let (row, payload) = build_row(
                &tx,
                self.blobs.as_ref(),
                &mut self.ids,
                &*self.clock,
                &head,
                event,
            )?;
            row.insert(&tx)?;
            head = Head {
                seq: row.seq,
                hash: row.hash.clone(),
            };
            let record = EventRecord::from_row(row)?;
            for projection in &self.projections {
                projection.apply(&tx, &record, &payload)?;
            }
            records.push(record);
        }
        for projection in &self.projections {
            projections::record_state(&tx, projection.as_ref(), head.seq)?;
        }
        tx.commit()?;
        self.head = head;
        // After the commit, so a woken subscriber finds the events stored.
        self.published.send_replace(self.head.seq);
        Ok(records)
    }
}

/// Runs `f`, turning a panic into an internal error.
fn guard<R>(f: impl FnOnce() -> Result<R>) -> Result<R> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err(StoreError::Internal("the writer panicked".into())))
}

/// Validates `event` and turns it into the row that follows `head`. Also
/// returns the full, canonical payload, which the row may only link to.
fn build_row(
    conn: &Connection,
    blobs: &dyn BlobStore,
    ids: &mut Generator,
    clock: &dyn Clock,
    head: &Head,
    event: NewEvent,
) -> Result<(Row, Value)> {
    let kind = events::kind(&event.event_type, event.version).ok_or_else(|| {
        events::EventError::UnknownType {
            event_type: event.event_type.clone(),
            version: event.version,
        }
    })?;
    let payload = kind.validate(&event.payload)?;
    exact_numbers(&payload).map_err(|reason| events::EventError::InvalidPayload {
        event_type: event.event_type.clone(),
        version: event.version,
        reason,
    })?;
    let canonical = serde_jcs::to_string(&payload)?;
    let stored = if canonical.len() > BLOB_THRESHOLD {
        let blob = blobs.put(conn, canonical.as_bytes())?;
        serde_jcs::to_string(&json!({BLOB_KEY: blob.hash, "size": blob.size}))?
    } else {
        canonical
    };

    let now = clock.now().to_offset(time::UtcOffset::UTC);
    let id = ids
        .generate_from_datetime(SystemTime::from(now))
        .map_err(|_| StoreError::Internal("ran out of ULIDs for this millisecond".into()))?;
    // ULIDs are unique and roughly ordered; the seq, not the id, is the order
    // (the generator starts afresh on reopen, and clocks can step back).
    let mut row = Row {
        seq: head.seq + 1,
        id: id.to_string(),
        ts: now.format(&Rfc3339)?,
        event_type: event.event_type,
        version: i64::from(event.version),
        stream: event.stream,
        correlation: event.correlation,
        causation: event.causation.map(|c| c.to_string()),
        actor: event.actor.to_string(),
        payload: stored,
        prev_hash: head.hash.clone(),
        hash: String::new(),
    };
    row.hash = row.expected_hash()?;
    Ok((row, payload))
}

/// Canonical JSON (RFC 8785) writes numbers as IEEE doubles, so whole
/// numbers beyond ±2^53 would be stored changed. They are refused instead.
fn exact_numbers(value: &Value) -> std::result::Result<(), String> {
    const LIMIT: u64 = 1 << 53;
    match value {
        Value::Number(n) => {
            // Integers must fit; so must floats, which past 2^53 are whole
            // numbers that canonical JSON writes as integers.
            let fits = n
                .as_u64()
                .map(|u| u <= LIMIT)
                .or_else(|| n.as_i64().map(|i| i.unsigned_abs() <= LIMIT))
                .or_else(|| n.as_f64().map(|f| f.abs() <= LIMIT as f64))
                .unwrap_or(true);
            if fits {
                Ok(())
            } else {
                Err(format!(
                    "{n} is too large to store exactly (the limit is 2^53)"
                ))
            }
        }
        Value::Array(items) => items.iter().try_for_each(exact_numbers),
        Value::Object(map) => map.values().try_for_each(exact_numbers),
        _ => Ok(()),
    }
}

/// Read-only connections, opened on demand and kept for reuse.
struct ReadPool {
    path: PathBuf,
    keep: usize,
    idle: Mutex<Vec<Connection>>,
}

impl ReadPool {
    fn new(path: &Path, keep: usize) -> Self {
        Self {
            path: path.to_path_buf(),
            keep: keep.max(1),
            idle: Mutex::new(Vec::new()),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&Connection) -> Result<R>) -> Result<R> {
        let idle = self.idle.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let conn = match idle {
            Some(conn) => conn,
            None => db::open(&self.path, Access::Read)?,
        };
        let result = f(&conn);
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        if idle.len() < self.keep {
            idle.push(conn);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use priya_api::events::{Actor, IntentAnswered, SettingChanged, SettingSource};
    use priya_testkit::FakeClock;

    use super::*;
    use crate::chain::ChainBreak;

    struct Fixture {
        _dir: tempfile::TempDir,
        config: StoreConfig,
        store: EventStore,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let config = StoreConfig::in_data_dir(dir.path());
        let store = EventStore::open(&config, Arc::new(FakeClock::new())).unwrap();
        Fixture {
            _dir: dir,
            config,
            store,
        }
    }

    fn setting(n: u32) -> NewEvent {
        NewEvent::new(
            "settings",
            Actor::User,
            &SettingChanged {
                key: format!("demo.key{n}"),
                old: None,
                new: Some(Value::from(n)),
                source: SettingSource::User,
            },
        )
        .unwrap()
    }

    async fn five_events(store: &EventStore) -> Vec<EventRecord> {
        store
            .append_all((1..=5).map(setting).collect())
            .await
            .unwrap()
    }

    /// A direct connection to the database, with the append-only triggers
    /// removed, as someone tampering with the file would do.
    fn tamper(config: &StoreConfig, sql: &str) {
        let conn = Connection::open(&config.db).unwrap();
        conn.execute_batch(&format!(
            "DROP TRIGGER IF EXISTS events_no_update; DROP TRIGGER IF EXISTS events_no_delete; {sql}"
        ))
        .unwrap();
    }

    async fn verify(store: &EventStore) -> ChainReport {
        store.verify_chain(1..=i64::MAX).await.unwrap()
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn append_assigns_monotonic_seq_and_links_hashes() {
        let f = fixture();
        let first = f.store.append(setting(0)).await.unwrap();
        assert_eq!(first.seq, 1);
        assert_eq!(first.prev_hash, GENESIS);
        let rest = five_events(&f.store).await;
        let seqs: Vec<i64> = rest.iter().map(|r| r.seq).collect();
        assert_eq!(seqs, [2, 3, 4, 5, 6]);
        assert_eq!(rest[0].prev_hash, first.hash);
        for pair in rest.windows(2) {
            assert_eq!(pair[1].prev_hash, pair[0].hash);
            assert!(pair[1].event.id > pair[0].event.id, "ULIDs increase");
        }
        assert_eq!(rest[4].event.actor, Actor::User);
        assert_eq!(rest[4].event.payload["setting"], "demo.key5");

        let read = f.store.read_after(0, 100).await.unwrap();
        assert_eq!(read.len(), 6);
        assert_eq!(read[5], rest[4], "what was written reads back the same");
        assert_eq!(f.store.read_after(4, 1).await.unwrap()[0].seq, 5);
        assert_eq!(
            verify(&f.store).await,
            ChainReport {
                ok: true,
                checked: 6,
                first_bad_seq: None,
                reason: None
            }
        );
    }

    // covers: CORE-01
    // suite: unit
    #[tokio::test]
    async fn stored_events_cross_the_api_unchanged() {
        use priya_api::methods::events::StoredEvent;
        let f = fixture();
        let record = f.store.append(setting(1)).await.unwrap();
        let wire = serde_json::to_value(StoredEvent::from(record.clone())).unwrap();
        let back: StoredEvent = serde_json::from_value(wire).unwrap();
        assert_eq!(
            (back.seq, back.event, back.prev_hash, back.hash),
            (record.seq, record.event, record.prev_hash, record.hash)
        );
    }

    // covers: CORE-01
    // suite: unit
    #[tokio::test]
    async fn a_reopened_store_continues_the_chain() {
        let dir = tempfile::tempdir().unwrap();
        let config = StoreConfig::in_data_dir(dir.path());
        let last = {
            let store = EventStore::open(&config, Arc::new(FakeClock::new())).unwrap();
            five_events(&store).await.pop().unwrap()
        };
        let store = EventStore::open(&config, Arc::new(FakeClock::new())).unwrap();
        let next = store.append(setting(6)).await.unwrap();
        assert_eq!(next.seq, 6);
        assert_eq!(next.prev_hash, last.hash);
        assert!(verify(&store).await.ok);
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn verify_detects_edited_payload() {
        let f = fixture();
        five_events(&f.store).await;
        tamper(
            &f.config,
            "UPDATE events SET payload = json_set(payload, '$.new', 99) WHERE seq = 3",
        );
        let report = verify(&f.store).await;
        assert_eq!(
            (report.first_bad_seq, report.reason),
            (Some(3), Some(ChainBreak::HashMismatch))
        );
        assert_eq!(report.checked, 2);
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn verify_detects_deleted_row() {
        let f = fixture();
        five_events(&f.store).await;
        tamper(&f.config, "DELETE FROM events WHERE seq = 3");
        let report = verify(&f.store).await;
        assert_eq!(
            (report.first_bad_seq, report.reason),
            (Some(3), Some(ChainBreak::SeqGap))
        );
        // Starting the check after the gap still notices it.
        let from_four = f.store.verify_chain(4..=5).await.unwrap();
        assert_eq!(from_four.first_bad_seq, Some(3));
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn verify_detects_reordered_rows() {
        let f = fixture();
        five_events(&f.store).await;
        // Swap rows 2 and 3 by renumbering them.
        tamper(
            &f.config,
            "UPDATE events SET seq = 100 WHERE seq = 2;
             UPDATE events SET seq = 2 WHERE seq = 3;
             UPDATE events SET seq = 3 WHERE seq = 100;",
        );
        let report = verify(&f.store).await;
        assert_eq!(
            (report.first_bad_seq, report.reason),
            (Some(2), Some(ChainBreak::PrevLinkBroken))
        );

        // Swapping just the payloads is caught too.
        let g = fixture();
        five_events(&g.store).await;
        tamper(
            &g.config,
            "UPDATE events SET payload = (SELECT payload FROM events WHERE seq = 4) WHERE seq = 2",
        );
        assert_eq!(verify(&g.store).await.first_bad_seq, Some(2));
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn editing_any_column_is_reported_at_its_seq() {
        let other_hash = "f".repeat(64);
        let edits = [
            ("seq", "seq = 1000".to_owned(), ChainBreak::SeqGap),
            (
                "id",
                "id = '01ZZZZZZZZZZZZZZZZZZZZZZZZ'".into(),
                ChainBreak::HashMismatch,
            ),
            (
                "ts",
                "ts = '2030-01-01T00:00:00Z'".into(),
                ChainBreak::HashMismatch,
            ),
            (
                "type",
                "type = 'setting.other'".into(),
                ChainBreak::HashMismatch,
            ),
            ("version", "version = 2".into(), ChainBreak::HashMismatch),
            (
                "stream",
                "stream = 'elsewhere'".into(),
                ChainBreak::HashMismatch,
            ),
            (
                "correlation",
                "correlation = 'task:x'".into(),
                ChainBreak::HashMismatch,
            ),
            (
                "causation",
                "causation = '01ZZZZZZZZZZZZZZZZZZZZZZZZ'".into(),
                ChainBreak::HashMismatch,
            ),
            ("actor", "actor = 'policy'".into(), ChainBreak::HashMismatch),
            ("payload", "payload = '{}'".into(), ChainBreak::HashMismatch),
            (
                "prev_hash",
                format!("prev_hash = '{other_hash}'"),
                ChainBreak::PrevLinkBroken,
            ),
            (
                "hash",
                format!("hash = '{other_hash}'"),
                ChainBreak::HashMismatch,
            ),
        ];
        for (column, set, reason) in edits {
            for target in [1, 3, 5] {
                let f = fixture();
                five_events(&f.store).await;
                tamper(
                    &f.config,
                    &format!("UPDATE events SET {set} WHERE seq = {target}"),
                );
                let report = verify(&f.store).await;
                assert_eq!(
                    (report.first_bad_seq, report.reason),
                    (Some(target), Some(reason)),
                    "editing {column} of row {target}"
                );
            }
        }
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn large_payload_goes_to_blob_and_round_trips() {
        let f = fixture();
        let answer = "A long answer. ".repeat(1_000);
        let event = NewEvent::new(
            "intent:01",
            Actor::PriyaAi,
            &IntentAnswered {
                answer: answer.clone(),
                model: "claude".into(),
            },
        )
        .unwrap();
        let record = f.store.append(event).await.unwrap();
        let hash = record.blob().expect("moved to a blob").to_owned();
        assert_eq!(hash.len(), 64);
        assert!(record.event.payload["size"].as_u64().unwrap() > BLOB_THRESHOLD as u64);
        let full = f.store.payload(&record).await.unwrap();
        assert_eq!(full["answer"], answer.as_str());

        // A small payload stays inline.
        let small = f.store.append(setting(1)).await.unwrap();
        assert!(small.blob().is_none());
        assert_eq!(f.store.payload(&small).await.unwrap(), small.event.payload);
        assert!(verify(&f.store).await.ok);
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn unknown_event_type_rejected() {
        let f = fixture();
        let mut unknown = setting(1);
        unknown.event_type = "task.created".into();
        let err = f.store.append(unknown).await.unwrap_err();
        assert!(
            matches!(
                err,
                StoreError::Event(events::EventError::UnknownType { .. })
            ),
            "{err}"
        );

        let mut bad = setting(1);
        bad.payload["unexpected"] = Value::from(true);
        let err = f.store.append(bad).await.unwrap_err();
        assert!(matches!(
            err,
            StoreError::Event(events::EventError::InvalidPayload { .. })
        ));
        assert!(
            f.store.read_after(0, 10).await.unwrap().is_empty(),
            "nothing written"
        );
    }

    // covers: CORE-01
    // suite: unit
    #[tokio::test]
    async fn a_batch_with_one_bad_event_writes_nothing() {
        let f = fixture();
        f.store.append(setting(0)).await.unwrap();
        let mut bad = setting(2);
        bad.version = 9;
        let err = f.store.append_all(vec![setting(1), bad, setting(3)]).await;
        assert!(err.is_err());
        assert_eq!(f.store.read_after(0, 10).await.unwrap().len(), 1);
        // The chain head didn't move: the next event links to the first.
        let next = f.store.append(setting(4)).await.unwrap();
        assert_eq!(next.seq, 2);
        assert!(verify(&f.store).await.ok);
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn numbers_too_large_to_store_exactly_are_refused() {
        let f = fixture();
        let event = |data: Value| {
            NewEvent::new(
                "module:weather",
                Actor::Module("weather".into()),
                &priya_api::events::ModuleEvent {
                    module: "weather".into(),
                    name: "reading".into(),
                    data,
                },
            )
            .unwrap()
        };
        let limit = 1_u64 << 53;
        f.store
            .append(event(json!({"n": limit, "m": -(limit as i64), "x": 1.5})))
            .await
            .unwrap();
        for too_big in [
            json!({"n": limit + 1}),
            json!([u64::MAX]),
            json!({"a": {"b": i64::MIN}}),
            json!({"f": 1e19}),
        ] {
            let err = f.store.append(event(too_big.clone())).await.unwrap_err();
            assert!(
                err.to_string().contains("too large to store exactly"),
                "{too_big}: {err}"
            );
        }
        assert_eq!(f.store.read_after(0, 10).await.unwrap().len(), 1);
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn verify_ranges_past_the_end_or_empty_are_fine() {
        let f = fixture();
        five_events(&f.store).await;
        let fine = ChainReport {
            ok: true,
            checked: 0,
            first_bad_seq: None,
            reason: None,
        };
        assert_eq!(f.store.verify_chain(10..=20).await.unwrap(), fine);
        assert_eq!(f.store.verify_chain(6..=9).await.unwrap(), fine);
        #[allow(clippy::reversed_empty_ranges)]
        let backwards = 7..=3;
        assert_eq!(f.store.verify_chain(backwards).await.unwrap(), fine);
        assert_eq!(f.store.verify_chain(4..=100).await.unwrap().checked, 2);
        assert!(
            fixture().store.verify_chain(1..=10).await.unwrap().ok,
            "an empty log"
        );
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[tokio::test]
    async fn deleting_the_newest_rows_is_noticed_after_the_next_append() {
        let dir = tempfile::tempdir().unwrap();
        let config = StoreConfig::in_data_dir(dir.path());
        {
            let store = EventStore::open(&config, Arc::new(FakeClock::new())).unwrap();
            five_events(&store).await;
        }
        tamper(&config, "DELETE FROM events WHERE seq >= 4");
        let store = EventStore::open(&config, Arc::new(FakeClock::new())).unwrap();
        let next = store.append(setting(9)).await.unwrap();
        assert_eq!(next.seq, 6, "seqs 4 and 5 are never reused");
        let report = verify(&store).await;
        assert_eq!(
            (report.first_bad_seq, report.reason),
            (Some(4), Some(ChainBreak::SeqGap))
        );
    }

    // covers: CORE-01
    // suite: unit
    #[tokio::test]
    async fn reads_beyond_the_pool_size_wait_their_turn() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = StoreConfig::in_data_dir(dir.path());
        config.readers = 1;
        let store = EventStore::open(&config, Arc::new(FakeClock::new())).unwrap();
        five_events(&store).await;
        let reads: Vec<_> = (0..32)
            .map(|_| {
                let store = store.clone();
                tokio::spawn(async move { store.read_after(0, 10).await.unwrap().len() })
            })
            .collect();
        for read in reads {
            assert_eq!(read.await.unwrap(), 5);
        }
    }
}
