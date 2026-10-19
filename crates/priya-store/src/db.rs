//! Opening the database and keeping its schema current.
//!
//! - Every connection gets the same pragmas: WAL, `synchronous=FULL` (an
//!   acknowledged event survives power loss, principle P9), foreign keys on,
//!   and a 5 s busy timeout.
//! - Migrations are forward-only and numbered by `PRAGMA user_version`.
//!   Before each one the database is copied to `<file>.bak-v<N>` with
//!   SQLite's online backup, so a failed upgrade can be undone by hand.
//! - A database newer than this build is refused with a plain message
//!   instead of being misread.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use tracing::info;

use crate::error::{Result, StoreError};

/// One forward-only schema step. `version` is the `user_version` after it.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: i64,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    sql: include_str!("../migrations/0001_init.sql"),
}];

/// How a connection will be used. There is one writer; readers can't write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Write,
    Read,
}

/// Opens the database with PriyaOS's settings. A writer creates the file
/// if needed; a reader expects it to exist.
pub fn open(path: &Path, access: Access) -> Result<Connection> {
    let flags = match access {
        Access::Write => OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        Access::Read => OpenFlags::SQLITE_OPEN_READ_ONLY,
    } | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(path, flags)?;
    if access == Access::Write {
        // WAL is a property of the file; set it once from the writer.
        conn.pragma_update(None, "journal_mode", "WAL")?;
    }
    conn.pragma_update(None, "synchronous", "FULL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

/// The schema versions before and after [`migrate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migrated {
    pub from: i64,
    pub to: i64,
}

/// Brings the database at `path` (open as `conn`) up to the newest schema.
pub fn migrate(conn: &mut Connection, path: &Path) -> Result<Migrated> {
    migrate_with(conn, path, MIGRATIONS)
}

fn migrate_with(conn: &mut Connection, path: &Path, migrations: &[Migration]) -> Result<Migrated> {
    let from: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let supported = migrations.last().map_or(0, |m| m.version);
    if from > supported {
        return Err(StoreError::NewerSchema {
            found: from,
            supported,
        });
    }
    for migration in migrations.iter().filter(|m| m.version > from) {
        let backup = backup_path(path, migration.version);
        backup_to(conn, &backup)?;
        info!(version = migration.version, backup = %backup.display(), "migrating the database");
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.pragma_update(None, "user_version", migration.version)?;
        tx.commit()?;
    }
    Ok(Migrated {
        from,
        to: supported.max(from),
    })
}

/// `priya.db` → `priya.db.bak-v3`.
pub fn backup_path(path: &Path, version: i64) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".bak-v{version}"));
    path.with_file_name(name)
}

/// A consistent copy of the live database, taken without stopping writers.
fn backup_to(conn: &Connection, target: &Path) -> Result<()> {
    let mut copy = Connection::open(target)?;
    let backup = rusqlite::backup::Backup::new(conn, &mut copy)?;
    backup.run_to_completion(256, std::time::Duration::ZERO, None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("priya.db");
        (dir, path)
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn connections_get_the_durability_settings_and_fts5() {
        let (_dir, path) = temp_db();
        let writer = open(&path, Access::Write).unwrap();
        let get = |conn: &Connection, pragma: &str| -> String {
            conn.pragma_query_value(None, pragma, |r| r.get::<_, rusqlite::types::Value>(0))
                .map(|v| format!("{v:?}"))
                .unwrap()
        };
        assert!(get(&writer, "journal_mode").contains("wal"));
        assert!(get(&writer, "synchronous").contains('2'), "FULL is 2");
        assert!(get(&writer, "foreign_keys").contains('1'));
        assert!(get(&writer, "busy_timeout").contains("5000"));
        writer
            .execute_batch("CREATE VIRTUAL TABLE t USING fts5(body)")
            .expect("bundled SQLite has FTS5");

        let reader = open(&path, Access::Read).unwrap();
        assert!(
            reader.execute_batch("CREATE TABLE x(a)").is_err(),
            "readers can't write"
        );
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    #[test]
    fn migration_creates_backup_and_sets_user_version() {
        let (_dir, path) = temp_db();
        let mut conn = open(&path, Access::Write).unwrap();
        assert_eq!(
            migrate(&mut conn, &path).unwrap(),
            Migrated { from: 0, to: 1 }
        );
        assert!(backup_path(&path, 1).exists());
        conn.execute_batch("INSERT INTO projection_state VALUES ('demo', 1, 7)")
            .unwrap();

        // A second, pretend migration backs up the data as it was before it.
        let next = [
            MIGRATIONS[0],
            Migration {
                version: 2,
                sql: "ALTER TABLE projection_state ADD COLUMN note TEXT",
            },
        ];
        assert_eq!(
            migrate_with(&mut conn, &path, &next).unwrap(),
            Migrated { from: 1, to: 2 }
        );
        let backup = Connection::open(backup_path(&path, 2)).unwrap();
        let (version, seq): (i64, i64) = (
            backup
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap(),
            backup
                .query_row("SELECT last_seq FROM projection_state", [], |r| r.get(0))
                .unwrap(),
        );
        assert_eq!((version, seq), (1, 7), "the backup is the v1 database");
        let now: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(now, 2);

        // Running again changes nothing.
        assert_eq!(
            migrate_with(&mut conn, &path, &next).unwrap(),
            Migrated { from: 2, to: 2 }
        );
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    #[test]
    fn refuses_newer_schema_version_with_plain_message() {
        let (_dir, path) = temp_db();
        let mut conn = open(&path, Access::Write).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        let err = migrate(&mut conn, &path).unwrap_err();
        assert!(matches!(
            err,
            StoreError::NewerSchema {
                found: 99,
                supported: 1
            }
        ));
        use priya_api::error::UserFacing;
        assert!(err.user_message().contains("newer version of PriyaOS"));
        assert!(!err.user_message().contains("schema"), "no jargon");
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn events_cannot_be_updated_or_deleted() {
        let (_dir, path) = temp_db();
        let mut conn = open(&path, Access::Write).unwrap();
        migrate(&mut conn, &path).unwrap();
        conn.execute_batch(
            "INSERT INTO events (id, ts, type, version, stream, actor, payload, prev_hash, hash)
             VALUES ('01', 't', 'x', 1, 's', 'user', '{}', 'p', 'h')",
        )
        .unwrap();
        for sql in [
            "UPDATE events SET payload = '{\"x\":1}'",
            "DELETE FROM events",
        ] {
            let err = conn.execute_batch(sql).unwrap_err().to_string();
            assert!(err.contains("append-only"), "{sql}: {err}");
        }
    }
}
