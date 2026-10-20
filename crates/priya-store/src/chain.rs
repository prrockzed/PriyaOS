//! The hash chain that makes the log tamper-evident.
//!
//! Each row's `hash` is `BLAKE3(prev_hash || JCS(row without hash))`, where
//! JCS is RFC 8785 canonical JSON: keys sorted, one number format, no
//! whitespace. So the hash depends on what the row says, never on how its
//! JSON happens to be spelled. The first row's `prev_hash` is 64 zeros.
//!
//! [`verify`] walks the rows in order and reports the first one that doesn't
//! fit: a changed column (`hash_mismatch`), a broken link (`prev_link_broken`)
//! or a missing row (`seq_gap`). Rows removed from the very end leave no trace
//! in the chain itself; checkpoints (Phase 3) cover that.

use std::ops::RangeInclusive;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::error::{Result, StoreError};

/// The `prev_hash` of the first event.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// An `events` row exactly as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub seq: i64,
    pub id: String,
    pub ts: String,
    pub event_type: String,
    pub version: i64,
    pub stream: String,
    pub correlation: Option<String>,
    pub causation: Option<String>,
    pub actor: String,
    /// JSON text.
    pub payload: String,
    pub prev_hash: String,
    pub hash: String,
}

impl Row {
    pub(crate) const COLUMNS: &'static str = "seq, id, ts, type, version, stream, correlation, causation, actor, payload, prev_hash, hash";

    pub(crate) fn from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            seq: row.get(0)?,
            id: row.get(1)?,
            ts: row.get(2)?,
            event_type: row.get(3)?,
            version: row.get(4)?,
            stream: row.get(5)?,
            correlation: row.get(6)?,
            causation: row.get(7)?,
            actor: row.get(8)?,
            payload: row.get(9)?,
            prev_hash: row.get(10)?,
            hash: row.get(11)?,
        })
    }

    pub(crate) fn insert(&self, conn: &Connection) -> Result<()> {
        conn.prepare_cached(
            "INSERT INTO events (seq, id, ts, type, version, stream, correlation, causation, actor, payload, prev_hash, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?
        .execute(params![
            self.seq,
            self.id,
            self.ts,
            self.event_type,
            self.version,
            self.stream,
            self.correlation,
            self.causation,
            self.actor,
            self.payload,
            self.prev_hash,
            self.hash,
        ])?;
        Ok(())
    }

    /// The hash this row should have, from everything but its `hash`.
    pub fn expected_hash(&self) -> Result<String> {
        let payload: Value = serde_json::from_str(&self.payload)
            .map_err(|e| StoreError::Corrupt(format!("event {}: payload: {e}", self.seq)))?;
        let content = json!({
            "seq": self.seq,
            "id": self.id,
            "ts": self.ts,
            "type": self.event_type,
            "version": self.version,
            "stream": self.stream,
            "correlation": self.correlation,
            "causation": self.causation,
            "actor": self.actor,
            "payload": payload,
            "prev_hash": self.prev_hash,
        });
        let canonical = serde_jcs::to_vec(&content)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.prev_hash.as_bytes());
        hasher.update(&canonical);
        Ok(hasher.finalize().to_hex().to_string())
    }
}

/// Why the chain broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainBreak {
    HashMismatch,
    PrevLinkBroken,
    SeqGap,
}

/// The result of [`verify`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChainReport {
    pub ok: bool,
    /// Rows checked before stopping.
    pub checked: u64,
    /// For a gap, the missing seq; otherwise the first row that doesn't fit.
    pub first_bad_seq: Option<i64>,
    pub reason: Option<ChainBreak>,
}

impl ChainReport {
    fn broken(checked: u64, seq: i64, reason: ChainBreak) -> Self {
        Self {
            ok: false,
            checked,
            first_bad_seq: Some(seq),
            reason: Some(reason),
        }
    }
}

/// Checks the rows whose seq is in `range` (ends past the last row are
/// fine), stopping at the first break.
pub fn verify(conn: &Connection, range: RangeInclusive<i64>) -> Result<ChainReport> {
    let start = (*range.start()).max(1);
    let end = *range.end();
    let last: i64 = conn.query_row("SELECT coalesce(max(seq), 0) FROM events", [], |r| r.get(0))?;
    if start > end || start > last {
        // Nothing stored in that range: nothing to check.
        return Ok(ChainReport {
            ok: true,
            checked: 0,
            first_bad_seq: None,
            reason: None,
        });
    }
    let mut expected_seq = start;
    let mut expected_prev = if start == 1 {
        GENESIS.to_owned()
    } else {
        let before: Option<String> = conn
            .query_row("SELECT hash FROM events WHERE seq = ?1", [start - 1], |r| {
                r.get(0)
            })
            .optional()?;
        match before {
            Some(hash) => hash,
            None => return Ok(ChainReport::broken(0, start - 1, ChainBreak::SeqGap)),
        }
    };

    let mut statement = conn.prepare(&format!(
        "SELECT {} FROM events WHERE seq BETWEEN ?1 AND ?2 ORDER BY seq",
        Row::COLUMNS
    ))?;
    let mut rows = statement.query([start, end])?;
    let mut checked = 0;
    while let Some(row) = rows.next()? {
        let row = Row::from_sql(row)?;
        if row.seq != expected_seq {
            return Ok(ChainReport::broken(
                checked,
                expected_seq,
                ChainBreak::SeqGap,
            ));
        }
        if row.prev_hash != expected_prev {
            return Ok(ChainReport::broken(
                checked,
                row.seq,
                ChainBreak::PrevLinkBroken,
            ));
        }
        // A payload that is no longer JSON is a changed row too.
        let matches = row.expected_hash().is_ok_and(|hash| hash == row.hash);
        if !matches {
            return Ok(ChainReport::broken(
                checked,
                row.seq,
                ChainBreak::HashMismatch,
            ));
        }
        checked += 1;
        expected_prev = row.hash;
        expected_seq += 1;
    }
    Ok(ChainReport {
        ok: true,
        checked,
        first_bad_seq: None,
        reason: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(payload: &str) -> Row {
        Row {
            seq: 1,
            id: "01JABCDEFGHJKMNPQRSTVWXYZ0".into(),
            ts: "2026-10-20T03:04:05Z".into(),
            event_type: "setting.changed".into(),
            version: 1,
            stream: "settings".into(),
            correlation: None,
            causation: None,
            actor: "user".into(),
            payload: payload.into(),
            prev_hash: GENESIS.into(),
            hash: String::new(),
        }
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[test]
    fn hash_is_independent_of_json_key_order() {
        let a = row(r#"{"key":"ai.strategy","new":"cheapest","old":null,"source":"user"}"#);
        let b =
            row(r#"{ "source": "user", "old": null, "new": "cheapest", "key": "ai.strategy" }"#);
        assert_eq!(a.expected_hash().unwrap(), b.expected_hash().unwrap());

        // Anything that changes the meaning changes the hash.
        let c = row(r#"{"key":"ai.strategy","new":"best","old":null,"source":"user"}"#);
        assert_ne!(a.expected_hash().unwrap(), c.expected_hash().unwrap());
        let mut d = a.clone();
        d.seq = 2;
        assert_ne!(a.expected_hash().unwrap(), d.expected_hash().unwrap());
        assert_eq!(a.expected_hash().unwrap().len(), 64);
    }
}
