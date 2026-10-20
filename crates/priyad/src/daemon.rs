//! The daemon's place in the system: one instance per user, a private
//! socket, and telling systemd how it is doing.

use std::ffi::OsString;
use std::fs::{self, DirBuilder, File};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use tokio::net::UnixListener;
use tracing::debug;

/// Where the daemon keeps its runtime files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFiles {
    /// `$XDG_RUNTIME_DIR/priya`, private to the user (0700).
    pub dir: PathBuf,
    pub socket: PathBuf,
    pub lock: PathBuf,
}

impl RuntimeFiles {
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            socket: dir.join("priyad.sock"),
            lock: dir.join("priyad.lock"),
        }
    }
}

/// Why the daemon couldn't take its place.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("another priyad is already running for this user")]
    AlreadyRunning,
    #[error(
        "{} must be a folder of your own; it is a link, a file, or another user's",
        path.display()
    )]
    NotPrivate { path: PathBuf },
    #[error(
        "the socket path {} is too long for a Unix socket; set PRIYA_RUNTIME_DIR to a shorter folder",
        path.display()
    )]
    SocketPathTooLong { path: PathBuf },
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
}

impl StartError {
    fn io(path: &Path) -> impl FnOnce(io::Error) -> Self + '_ {
        move |source| Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// Holds the single-instance lock while it lives.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

/// The user the daemon runs as.
pub fn own_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// Makes the runtime folder (created private, 0700), checks it is a real
/// folder of this user's, and takes the single-instance lock.
pub fn take_place(files: &RuntimeFiles) -> Result<InstanceLock, StartError> {
    let created = DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&files.dir);
    match created {
        // Something that isn't a folder is caught just below.
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(StartError::io(&files.dir)(e)),
    }
    // Not following links: a link planted here could point anywhere.
    let found = fs::symlink_metadata(&files.dir).map_err(StartError::io(&files.dir))?;
    if !found.is_dir() || found.uid() != own_uid() {
        return Err(StartError::NotPrivate {
            path: files.dir.clone(),
        });
    }
    fs::set_permissions(&files.dir, fs::Permissions::from_mode(0o700))
        .map_err(StartError::io(&files.dir))?;
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&files.lock)
        .map_err(StartError::io(&files.lock))?;
    match file.try_lock() {
        Ok(()) => Ok(InstanceLock { _file: file }),
        Err(std::fs::TryLockError::WouldBlock) => Err(StartError::AlreadyRunning),
        Err(std::fs::TryLockError::Error(e)) => Err(StartError::io(&files.lock)(e)),
    }
}

/// Binds the socket (replacing a stale one: the lock says no other daemon
/// owns it) with mode 0600. The folder is private already, so nobody can
/// reach the socket before its mode is set.
pub fn bind(files: &RuntimeFiles, _lock: &InstanceLock) -> Result<UnixListener, StartError> {
    match fs::remove_file(&files.socket) {
        Ok(()) => debug!("removed a stale socket"),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(StartError::io(&files.socket)(e)),
    }
    let listener = UnixListener::bind(&files.socket).map_err(|e| {
        // The standard library refuses paths over the platform's limit
        // (104 bytes on macOS, 108 on Linux) before asking the kernel.
        if e.kind() == io::ErrorKind::InvalidInput && files.socket.as_os_str().len() >= 104 {
            StartError::SocketPathTooLong {
                path: files.socket.clone(),
            }
        } else {
            StartError::io(&files.socket)(e)
        }
    })?;
    fs::set_permissions(&files.socket, fs::Permissions::from_mode(0o600))
        .map_err(StartError::io(&files.socket))?;
    Ok(listener)
}

/// Tells systemd how the daemon is doing (`READY=1`, `STOPPING=1`) when it
/// runs under systemd with `Type=notify`; otherwise does nothing.
#[derive(Debug, Clone, Default)]
pub struct Systemd {
    target: Option<OsString>,
}

impl Systemd {
    /// Reads `NOTIFY_SOCKET` once, at start-up.
    pub fn from_env() -> Self {
        Self {
            target: std::env::var_os("NOTIFY_SOCKET"),
        }
    }

    pub fn notify(&self, state: &str) {
        let Some(target) = &self.target else {
            return;
        };
        if let Err(e) = send_notify(target, state) {
            debug!(error = %e, "couldn't notify systemd");
        }
    }
}

fn send_notify(target: &std::ffi::OsStr, state: &str) -> io::Result<()> {
    use std::os::unix::net::UnixDatagram;
    let socket = UnixDatagram::unbound()?;
    let target = target.to_string_lossy();
    if let Some(name) = target.strip_prefix('@') {
        // An abstract socket name, as systemd uses on Linux.
        #[cfg(target_os = "linux")]
        {
            use std::os::linux::net::SocketAddrExt;
            let address = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes())?;
            socket.send_to_addr(state.as_bytes(), &address)?;
            return Ok(());
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = name;
            return Err(io::Error::other("abstract sockets need Linux"));
        }
    }
    socket.send_to(state.as_bytes(), target.as_ref())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    // covers: CORE-04
    // suite: unit
    #[tokio::test]
    async fn one_instance_with_a_private_socket() {
        let root = tempfile::tempdir().unwrap();
        let files = RuntimeFiles::in_dir(&root.path().join("priya"));
        let lock = take_place(&files).unwrap();
        assert!(matches!(
            take_place(&files),
            Err(StartError::AlreadyRunning)
        ));
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&files.dir), 0o700);

        // A stale socket from a crash is replaced.
        fs::write(&files.socket, "stale").unwrap();
        let _listener = bind(&files, &lock).unwrap();
        assert_eq!(mode(&files.socket), 0o600);
        assert_eq!(fs::metadata(&files.socket).unwrap().uid(), own_uid());

        drop(lock);
        assert!(
            take_place(&files).is_ok(),
            "free again once the first is gone"
        );
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn the_runtime_folder_must_be_a_real_folder() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = root.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        let link = root.path().join("priya");
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        assert!(matches!(
            take_place(&RuntimeFiles::in_dir(&link)),
            Err(StartError::NotPrivate { .. })
        ));
        let file = root.path().join("file");
        fs::write(&file, "").unwrap();
        assert!(matches!(
            take_place(&RuntimeFiles::in_dir(&file)),
            Err(StartError::NotPrivate { .. })
        ));
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn a_socket_path_too_long_says_what_to_do() {
        let root = tempfile::tempdir().unwrap();
        let files = RuntimeFiles::in_dir(&root.path().join("d".repeat(120)));
        let lock = take_place(&files).unwrap();
        let err = bind(&files, &lock).unwrap_err();
        assert!(matches!(err, StartError::SocketPathTooLong { .. }), "{err}");
        assert!(err.to_string().contains("PRIYA_RUNTIME_DIR"));
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn systemd_is_told_over_its_socket() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("notify");
        let listener = std::os::unix::net::UnixDatagram::bind(&path).unwrap();
        let systemd = Systemd {
            target: Some(path.clone().into()),
        };
        systemd.notify("READY=1");
        let mut buffer = [0_u8; 32];
        let n = listener.recv(&mut buffer).unwrap();
        assert_eq!(&buffer[..n], b"READY=1");
        // Outside systemd it does nothing.
        Systemd::default().notify("READY=1");
    }
}
