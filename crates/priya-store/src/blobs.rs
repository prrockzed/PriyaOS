//! Large payloads, stored outside the event table (sub-phase 0.5).
//!
//! A blob is named by the BLAKE3 hash of its bytes, so the same content is
//! stored once. Each file is zstd-compressed (level 3) and written safely:
//! temporary file, fsync, rename, fsync of the folder. How many events point
//! at a blob is counted in the `blobs` table, inside the caller's transaction,
//! so the count always agrees with the events. Garbage collection at startup
//! deletes blobs nobody points at, and files a crash left without a row.
//!
//! `LocalBlobStore` keeps them in `$XDG_DATA_HOME/priya/blobs/<2 hex>/<hash>`.
//! Phase 2 moves them to `/var/lib/priya/blobs/<uid>/` behind priya-helper.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use priya_core::clock::Clock;
use rusqlite::{Connection, OptionalExtension, params};
use time::format_description::well_known::Rfc3339;

use crate::error::{Result, StoreError};

const ZSTD_LEVEL: i32 = 3;
const TEMP_PREFIX: &str = ".tmp-";

/// Where a blob is and how big it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRef {
    /// BLAKE3 of the uncompressed bytes, as 64 hex characters.
    pub hash: String,
    /// Uncompressed size in bytes.
    pub size: u64,
}

/// A content-addressed store whose reference counts live in the event
/// database, so they change in the same transaction as the events.
pub trait BlobStore: Send + Sync {
    /// Stores `data` if it isn't stored yet, and adds one reference.
    fn put(&self, conn: &Connection, data: &[u8]) -> Result<BlobRef>;
    /// The bytes of a blob, checked against its hash.
    fn get(&self, hash: &str) -> Result<Vec<u8>>;
    fn incref(&self, conn: &Connection, hash: &str) -> Result<()>;
    fn decref(&self, conn: &Connection, hash: &str) -> Result<()>;
    /// Deletes unreferenced blobs and leftover files; returns how many.
    fn gc(&self, conn: &Connection) -> Result<usize>;
}

pub struct LocalBlobStore {
    dir: PathBuf,
    clock: Arc<dyn Clock>,
}

impl LocalBlobStore {
    pub fn new(dir: impl Into<PathBuf>, clock: Arc<dyn Clock>) -> Self {
        Self {
            dir: dir.into(),
            clock,
        }
    }

    fn path(&self, hash: &str) -> PathBuf {
        self.dir.join(&hash[..2.min(hash.len())]).join(hash)
    }

    /// Writes the compressed bytes so a crash leaves either the whole file
    /// or none of it.
    fn write_file(&self, hash: &str, data: &[u8]) -> Result<u64> {
        let path = self.path(hash);
        let dir = path.parent().unwrap_or(&self.dir).to_path_buf();
        fs::create_dir_all(&dir).map_err(StoreError::io(&dir))?;
        let compressed = zstd::encode_all(data, ZSTD_LEVEL).map_err(StoreError::io(&path))?;
        let temp = dir.join(format!("{TEMP_PREFIX}{hash}"));
        let mut file = fs::File::create(&temp).map_err(StoreError::io(&temp))?;
        file.write_all(&compressed).map_err(StoreError::io(&temp))?;
        file.sync_all().map_err(StoreError::io(&temp))?;
        fs::rename(&temp, &path).map_err(StoreError::io(&path))?;
        sync_dir(&dir)?;
        Ok(compressed.len() as u64)
    }
}

impl BlobStore for LocalBlobStore {
    fn put(&self, conn: &Connection, data: &[u8]) -> Result<BlobRef> {
        let hash = blake3::hash(data).to_hex().to_string();
        let size = data.len() as u64;
        let known: Option<i64> = conn
            .query_row("SELECT refcount FROM blobs WHERE hash = ?1", [&hash], |r| {
                r.get(0)
            })
            .optional()?;
        if known.is_some() {
            // Rewrite the file if it went missing; the row says it should be.
            if !self.path(&hash).exists() {
                self.write_file(&hash, data)?;
            }
            self.incref(conn, &hash)?;
        } else {
            let stored = self.write_file(&hash, data)?;
            let created = self
                .clock
                .now()
                .format(&Rfc3339)
                .unwrap_or_else(|_| String::from("unknown"));
            conn.execute(
                "INSERT INTO blobs (hash, size, stored_size, refcount, created) VALUES (?1, ?2, ?3, 1, ?4)",
                params![hash, size as i64, stored as i64, created],
            )?;
        }
        Ok(BlobRef { hash, size })
    }

    fn get(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.path(hash);
        let bad = |reason: String| StoreError::BadBlob {
            hash: hash.to_owned(),
            reason,
        };
        let compressed = fs::read(&path).map_err(|e| bad(e.to_string()))?;
        let data = zstd::decode_all(compressed.as_slice()).map_err(|e| bad(e.to_string()))?;
        if blake3::hash(&data).to_hex().as_str() != hash {
            return Err(bad("its content doesn't match its hash".into()));
        }
        Ok(data)
    }

    fn incref(&self, conn: &Connection, hash: &str) -> Result<()> {
        change_refcount(conn, hash, 1)
    }

    fn decref(&self, conn: &Connection, hash: &str) -> Result<()> {
        change_refcount(conn, hash, -1)
    }

    fn gc(&self, conn: &Connection) -> Result<usize> {
        let mut removed = 0;
        let unreferenced: Vec<String> = conn
            .prepare("SELECT hash FROM blobs WHERE refcount = 0")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for hash in &unreferenced {
            remove_if_present(&self.path(hash))?;
            conn.execute("DELETE FROM blobs WHERE hash = ?1", [hash])?;
            removed += 1;
        }
        // Files without a row: temporary files and blobs whose transaction
        // never committed.
        let mut known = conn.prepare("SELECT 1 FROM blobs WHERE hash = ?1")?;
        for file in files_in(&self.dir)? {
            let name = file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if name.starts_with(TEMP_PREFIX) || !known.exists([&name])? {
                remove_if_present(&file)?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn change_refcount(conn: &Connection, hash: &str, by: i64) -> Result<()> {
    let changed = conn.execute(
        "UPDATE blobs SET refcount = refcount + ?1 WHERE hash = ?2",
        params![by, hash],
    )?;
    if changed == 0 {
        return Err(StoreError::BadBlob {
            hash: hash.to_owned(),
            reason: "no such blob".into(),
        });
    }
    Ok(())
}

/// The files one level below each `<2 hex>` folder.
fn files_in(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let Ok(shards) = fs::read_dir(dir) else {
        return Ok(files); // nothing stored yet
    };
    for shard in shards {
        let shard = shard.map_err(StoreError::io(dir))?.path();
        if shard.is_dir() {
            for entry in fs::read_dir(&shard).map_err(StoreError::io(&shard))? {
                files.push(entry.map_err(StoreError::io(&shard))?.path());
            }
        }
    }
    Ok(files)
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(StoreError::io(path)(e)),
    }
}

/// Makes a rename inside `dir` durable.
fn sync_dir(dir: &Path) -> Result<()> {
    fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(StoreError::io(dir))
}

#[cfg(test)]
mod tests {
    use priya_core::clock::SystemClock;

    use super::*;
    use crate::db::{Access, migrate, open};

    struct Fixture {
        _dir: tempfile::TempDir,
        conn: Connection,
        store: LocalBlobStore,
        blobs: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("priya.db");
        let mut conn = open(&db, Access::Write).unwrap();
        migrate(&mut conn, &db).unwrap();
        let blobs = dir.path().join("blobs");
        let store = LocalBlobStore::new(&blobs, Arc::new(SystemClock));
        Fixture {
            _dir: dir,
            conn,
            store,
            blobs,
        }
    }

    fn refcount(conn: &Connection, hash: &str) -> Option<i64> {
        conn.query_row("SELECT refcount FROM blobs WHERE hash = ?1", [hash], |r| {
            r.get(0)
        })
        .optional()
        .unwrap()
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    #[test]
    fn blob_put_is_idempotent_and_refcounted() {
        let f = fixture();
        let data = "a long answer ".repeat(1_000);
        let first = f.store.put(&f.conn, data.as_bytes()).unwrap();
        let second = f.store.put(&f.conn, data.as_bytes()).unwrap();
        assert_eq!(first, second, "same content, same blob");
        assert_eq!(first.size, data.len() as u64);
        assert_eq!(refcount(&f.conn, &first.hash), Some(2));
        assert_eq!(f.store.get(&first.hash).unwrap(), data.as_bytes());

        let on_disk = fs::metadata(f.store.path(&first.hash)).unwrap().len();
        assert!(
            on_disk < data.len() as u64 / 10,
            "compressed: {on_disk} bytes"
        );

        f.store.decref(&f.conn, &first.hash).unwrap();
        assert_eq!(refcount(&f.conn, &first.hash), Some(1));
        f.store.incref(&f.conn, &first.hash).unwrap();
        assert_eq!(refcount(&f.conn, &first.hash), Some(2));
        assert!(
            f.store.incref(&f.conn, &"0".repeat(64)).is_err(),
            "unknown blob"
        );
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    #[test]
    fn blob_gc_removes_only_zero_refcount() {
        let f = fixture();
        let kept = f.store.put(&f.conn, b"still used").unwrap();
        let dropped = f.store.put(&f.conn, b"no longer used").unwrap();
        f.store.decref(&f.conn, &dropped.hash).unwrap();

        // Leftovers of a crash: a temporary file and a file with no row.
        let shard = f.blobs.join("ab");
        fs::create_dir_all(&shard).unwrap();
        fs::write(shard.join(format!("{TEMP_PREFIX}abc")), b"half").unwrap();
        fs::write(shard.join("ab".repeat(32)), b"orphan").unwrap();

        assert_eq!(f.store.gc(&f.conn).unwrap(), 3);
        assert_eq!(f.store.get(&kept.hash).unwrap(), b"still used");
        assert!(f.store.get(&dropped.hash).is_err());
        assert_eq!(refcount(&f.conn, &dropped.hash), None);
        assert_eq!(fs::read_dir(&shard).unwrap().count(), 0);
        assert_eq!(f.store.gc(&f.conn).unwrap(), 0, "nothing left to collect");
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn damaged_or_missing_blobs_are_reported_and_repaired_on_put() {
        let f = fixture();
        let blob = f.store.put(&f.conn, b"precious").unwrap();
        let path = f.store.path(&blob.hash);

        fs::write(&path, zstd::encode_all(&b"tampered"[..], 3).unwrap()).unwrap();
        let err = f.store.get(&blob.hash).unwrap_err();
        assert!(err.to_string().contains("doesn't match its hash"), "{err}");

        fs::remove_file(&path).unwrap();
        assert!(matches!(
            f.store.get(&blob.hash),
            Err(StoreError::BadBlob { .. })
        ));
        f.store.put(&f.conn, b"precious").unwrap();
        assert_eq!(f.store.get(&blob.hash).unwrap(), b"precious", "rewritten");
        assert_eq!(refcount(&f.conn, &blob.hash), Some(2));
    }
}
