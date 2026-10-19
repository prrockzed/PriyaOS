-- Sub-phase 0.5 (CORE-01): the event log, projection bookkeeping and blobs.
-- Design: docs/claude/architecture/02-core.md §1. Tables are STRICT, so a
-- wrong type is an error instead of being stored as something else.

-- Append-only and hash-chained: hash = BLAKE3(prev_hash || JCS(row without
-- hash)). Events are never updated or deleted; retention compaction
-- (ADR-019) is the only exception and lifts the triggers below on purpose.
-- AUTOINCREMENT: a seq is never reused, even after the newest row is gone,
-- so a subscriber's stored "last seq" can't point at a different event.
CREATE TABLE events (
  seq          INTEGER PRIMARY KEY AUTOINCREMENT,  -- global order
  id           TEXT NOT NULL UNIQUE,               -- ULID
  ts           TEXT NOT NULL,                      -- RFC 3339 UTC
  type         TEXT NOT NULL,                      -- e.g. 'setting.changed'
  version      INTEGER NOT NULL CHECK (version >= 1),
  stream       TEXT NOT NULL,                      -- aggregate id, e.g. 'intent:01J…'
  correlation  TEXT,                               -- the task or conversation it belongs to
  causation    TEXT,                               -- the event that caused this one
  actor        TEXT NOT NULL,                      -- 'user' | 'priya-ai' | 'agent:<n>' | 'module:<n>' | 'policy' | 'system'
  payload      TEXT NOT NULL CHECK (json_valid(payload)),
  prev_hash    TEXT NOT NULL CHECK (length(prev_hash) = 64),  -- 64 zeros for the first event
  hash         TEXT NOT NULL CHECK (length(hash) = 64)
) STRICT;
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
  version   INTEGER NOT NULL CHECK (version >= 1),
  last_seq  INTEGER NOT NULL CHECK (last_seq >= 0)
) STRICT;

-- Large payloads, stored zstd-compressed under their BLAKE3 hash.
CREATE TABLE blobs (
  hash         TEXT PRIMARY KEY CHECK (length(hash) = 64),  -- BLAKE3 of the uncompressed bytes
  size         INTEGER NOT NULL CHECK (size >= 0),          -- uncompressed bytes
  stored_size  INTEGER NOT NULL CHECK (stored_size >= 0),   -- bytes on disk
  refcount     INTEGER NOT NULL CHECK (refcount >= 0),
  created      TEXT NOT NULL                                -- RFC 3339 UTC
) STRICT;
