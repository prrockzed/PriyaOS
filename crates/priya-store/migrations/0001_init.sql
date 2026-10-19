-- Sub-phase 0.5 (CORE-01): the event log, projection bookkeeping and blobs.
-- Design: docs/claude/architecture/02-core.md §1.

-- Append-only and hash-chained: hash = BLAKE3(prev_hash || JCS(row without
-- hash)). Events are never updated or deleted; retention compaction
-- (ADR-019) is the only exception and lifts the triggers below on purpose.
CREATE TABLE events (
  seq          INTEGER PRIMARY KEY,          -- global order
  id           TEXT NOT NULL UNIQUE,         -- ULID
  ts           TEXT NOT NULL,                -- RFC 3339 UTC
  type         TEXT NOT NULL,                -- e.g. 'setting.changed'
  version      INTEGER NOT NULL,             -- schema version of this event type
  stream       TEXT NOT NULL,                -- aggregate id, e.g. 'intent:01J…'
  correlation  TEXT,                         -- the task or conversation it belongs to
  causation    TEXT,                         -- the event that caused this one
  actor        TEXT NOT NULL,                -- 'user' | 'priya-ai' | 'agent:<n>' | 'module:<n>' | 'policy' | 'system'
  payload      TEXT NOT NULL,                -- JSON, validated against the type's struct
  prev_hash    TEXT NOT NULL,                -- the previous row's hash; 64 zeros for the first
  hash         TEXT NOT NULL
);
CREATE INDEX events_stream ON events(stream, seq);
CREATE INDEX events_type ON events(type, seq);

CREATE TRIGGER events_no_update BEFORE UPDATE ON events
BEGIN
  SELECT RAISE(ABORT, 'events are append-only');
END;
CREATE TRIGGER events_no_delete BEFORE DELETE ON events
BEGIN
  SELECT RAISE(ABORT, 'events are append-only');
END;

-- Which version of each projection is built, and up to which event.
CREATE TABLE projection_state (
  name      TEXT PRIMARY KEY,
  version   INTEGER NOT NULL,
  last_seq  INTEGER NOT NULL
);

-- Large payloads, stored zstd-compressed under their BLAKE3 hash.
CREATE TABLE blobs (
  hash         TEXT PRIMARY KEY,             -- BLAKE3 of the uncompressed bytes
  size         INTEGER NOT NULL,             -- uncompressed bytes
  stored_size  INTEGER NOT NULL,             -- bytes on disk
  refcount     INTEGER NOT NULL CHECK (refcount >= 0),
  created      TEXT NOT NULL                 -- RFC 3339 UTC
);
