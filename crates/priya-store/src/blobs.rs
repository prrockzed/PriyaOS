//! Large payloads, stored outside the event table (sub-phase 0.5).
//!
//! A blob is named by the BLAKE3 hash of its bytes, so the same content is
//! stored once. Each file is zstd-compressed (level 3) and written safely:
//! temporary file, fsync, rename, fsync of the folder. How many events point
//! at a blob is counted in the `blobs` table, inside the caller's transaction,
//! so the count always agrees with the events. Garbage collection deletes
//! blobs nobody points at, and files a crash left without a row; it runs at
//! startup, before the writer, because a `put` writes its file before the
//! caller's transaction commits.
//!
//! Hashes arrive in event payloads, which can be tampered with, so every
//! hash is checked to be 64 lowercase hex characters before it touches a
//! path.
//!
//! `LocalBlobStore` keeps them in `$XDG_DATA_HOME/priya/blobs/<2 hex>/<hash>`.
//! Phase 2 moves them to `/var/lib/priya/blobs/<uid>/` behind priya-helper.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
    /// Deletes unreferenced blobs and leftover files; returns how many. Run
    /// it only while nothing else writes (at startup, before the writer).
    fn gc(&self, conn: &Connection) -> Result<usize>;
}

pub struct LocalBlobStore {
    dir: PathBuf,
    clock: Arc<dyn Clock>,
    /// Makes each temporary file name unique, even for the same content.
    temp_counter: AtomicU64,
}

impl LocalBlobStore {
    pub fn new(dir: impl Into<PathBuf>, clock: Arc<dyn Clock>) -> Self {
        Self {
            dir: dir.into(),
            clock,
            temp_counter: AtomicU64::new(0),
        }
    }

    /// The file of a checked hash.
    fn path(&self, hash: &str) -> PathBuf {
        self.dir.join(&hash[..2]).join(hash)
    }

    /// Writes the compressed bytes so a crash leaves either the whole file
    /// or none of it.
    fn write_file(&self, hash: &str, data: &[u8]) -> Result<u64> {
        let path = self.path(hash);
        let dir = path.parent().unwrap_or(&self.dir).to_path_buf();
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(StoreError::io(&dir))?;
            // A new shard folder must survive a crash too.
            sync_dir(&self.dir)?;
        }
        let compressed = zstd::encode_all(data, ZSTD_LEVEL).map_err(StoreError::io(&path))?;
        let n = self.temp_counter.fetch_add(1, Ordering::Relaxed);
        let temp = dir.join(format!("{TEMP_PREFIX}{}-{n}-{hash}", std::process::id()));
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
        let known = conn
            .prepare_cached("SELECT 1 FROM blobs WHERE hash = ?1")?
            .exists([&hash])?;
        if known {
            // Rewrite the file if it went missing; the row says it should be.
            if !self.path(&hash).exists() {
                self.write_file(&hash, data)?;
            }
            self.incref(conn, &hash)?;
        } else {
            let stored = self.write_file(&hash, data)?;
            let created = self.clock.now().format(&Rfc3339)?;
            conn.execute(
                "INSERT INTO blobs (hash, size, stored_size, refcount, created) VALUES (?1, ?2, ?3, 1, ?4)",
                params![hash, size as i64, stored as i64, created],
            )?;
        }
        Ok(BlobRef { hash, size })
    }

    fn get(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.path(checked(hash)?);
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
            if let Ok(hash) = checked(hash) {
                remove_if_present(&self.path(hash))?;
            }
            conn.execute("DELETE FROM blobs WHERE hash = ?1", [hash])?;
            removed += 1;
        }
        // Files without a row: temporary files and blobs whose transaction
        // never committed. Only files this store could have written are ever
        // deleted.
        let mut known = conn.prepare("SELECT 1 FROM blobs WHERE hash = ?1")?;
        for file in files_in(&self.dir)? {
            let name = file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let leftover = name.starts_with(TEMP_PREFIX)
                || (checked(&name).is_ok() && !known.exists([&name])?);
            if leftover {
                remove_if_present(&file)?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// The hash, if it is 64 lowercase hex characters: the only names this
/// store writes, and the only ones safe to turn into a path.
fn checked(hash: &str) -> Result<&str> {
    let valid = hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if valid {
        Ok(hash)
    } else {
        Err(StoreError::BadBlob {
            hash: hash.chars().take(80).collect(),
            reason: "not a blob hash".into(),
        })
    }
}

fn change_refcount(conn: &Connection, hash: &str, by: i64) -> Result<()> {
    let hash = checked(hash)?;
    let misuse = |reason: &str| StoreError::BlobMisuse {
        hash: hash.to_owned(),
        reason: reason.into(),
    };
    let refcount: Option<i64> = conn
        .prepare_cached("SELECT refcount FROM blobs WHERE hash = ?1")?
        .query_row([hash], |r| r.get(0))
        .optional()?;
    match refcount {
        None => Err(misuse("no such blob")),
        Some(n) if n + by < 0 => Err(misuse("it has no references left")),
        Some(_) => {
            conn.execute(
                "UPDATE blobs SET refcount = refcount + ?1 WHERE hash = ?2",
                params![by, hash],
            )?;
            Ok(())
        }
    }
}

/// The regular files in each `<2 hex>` shard folder. Symlinks are never
/// followed and anything else is left alone, so `gc` can't reach outside the
/// store.
fn files_in(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let Ok(shards) = fs::read_dir(dir) else {
        return Ok(files); // nothing stored yet
    };
    for shard in shards {
        let shard = shard.map_err(StoreError::io(dir))?;
        let name = shard.file_name().to_string_lossy().into_owned();
        let is_shard = shard.file_type().map_err(StoreError::io(dir))?.is_dir()
            && name.len() == 2
            && name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if !is_shard {
            continue;
        }
        let shard = shard.path();
        for entry in fs::read_dir(&shard).map_err(StoreError::io(&shard))? {
            let entry = entry.map_err(StoreError::io(&shard))?;
            if entry.file_type().map_err(StoreError::io(&shard))?.is_file() {
                files.push(entry.path());
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

/// Makes a rename or a new folder inside `dir` durable.
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
        dir: tempfile::TempDir,
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
            dir,
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
    // regression: 0.5
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
        assert!(matches!(
            f.store.incref(&f.conn, &"0".repeat(64)),
            Err(StoreError::BlobMisuse { .. })
        ));
        f.store.decref(&f.conn, &first.hash).unwrap();
        f.store.decref(&f.conn, &first.hash).unwrap();
        let err = f.store.decref(&f.conn, &first.hash).unwrap_err();
        assert!(err.to_string().contains("no references left"), "{err}");
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[test]
    fn blob_gc_removes_only_zero_refcount() {
        let f = fixture();
        let kept = f.store.put(&f.conn, b"still used").unwrap();
        let dropped = f.store.put(&f.conn, b"no longer used").unwrap();
        f.store.decref(&f.conn, &dropped.hash).unwrap();

        // Leftovers of a crash: a temporary file and a blob file with no row.
        let shard = f.blobs.join("ab");
        fs::create_dir_all(&shard).unwrap();
        fs::write(shard.join(format!("{TEMP_PREFIX}1-0-abc")), b"half").unwrap();
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
    // acceptance: 0.5
    // regression: 0.5
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

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[test]
    fn hashes_from_payloads_cannot_escape_the_store() {
        let f = fixture();
        let upper = "A".repeat(64);
        let short = "0".repeat(63);
        for bad in ["aé", "../../etc/passwd", "/etc/passwd", "", &upper, &short] {
            assert!(
                matches!(f.store.get(bad), Err(StoreError::BadBlob { .. })),
                "{bad}"
            );
            assert!(f.store.decref(&f.conn, bad).is_err(), "{bad}");
        }
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[test]
    fn gc_never_follows_links_or_touches_foreign_files() {
        let f = fixture();
        f.store.put(&f.conn, b"kept").unwrap();
        // A folder outside the store, linked in as if it were a shard.
        let outside = f.dir.path().join("victim");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("important.txt"), b"keep me").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, f.blobs.join("cd")).unwrap();
        // Things a person or another tool left in the store.
        fs::create_dir_all(f.blobs.join("ef").join("subfolder")).unwrap();
        fs::write(f.blobs.join("ef").join("notes.txt"), b"mine").unwrap();
        fs::write(f.blobs.join("README"), b"mine").unwrap();

        assert_eq!(f.store.gc(&f.conn).unwrap(), 0);
        assert!(outside.join("important.txt").exists());
        assert!(f.blobs.join("ef").join("notes.txt").exists());
        assert!(f.blobs.join("README").exists());
    }

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    // regression: 0.5
    #[test]
    fn concurrent_puts_of_the_same_content_stay_whole() {
        let f = fixture();
        let data = "same content ".repeat(10_000);
        let hash = blake3::hash(data.as_bytes()).to_hex().to_string();
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| f.store.write_file(&hash, data.as_bytes()).unwrap());
            }
        });
        let compressed = fs::read(f.store.path(&hash)).unwrap();
        assert_eq!(
            zstd::decode_all(compressed.as_slice()).unwrap(),
            data.as_bytes()
        );
        let shard = f.store.path(&hash);
        let leftovers = fs::read_dir(shard.parent().unwrap()).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary files left behind");
    }
}
