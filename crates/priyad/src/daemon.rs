//! The daemon's place in the system: one instance per user, a private
//! socket, and telling systemd how it is doing.

use std::fs::{self, File};
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
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

/// Makes the runtime folder private and takes the single-instance lock.
pub fn take_place(files: &RuntimeFiles) -> Result<InstanceLock, StartError> {
    fs::create_dir_all(&files.dir).map_err(StartError::io(&files.dir))?;
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
/// owns it) with mode 0600. Returns it and the daemon's own uid, which is
/// the socket's owner.
pub fn bind(files: &RuntimeFiles, _lock: &InstanceLock) -> Result<(UnixListener, u32), StartError> {
    match fs::remove_file(&files.socket) {
        Ok(()) => debug!("removed a stale socket"),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(StartError::io(&files.socket)(e)),
    }
    let listener = UnixListener::bind(&files.socket).map_err(StartError::io(&files.socket))?;
    fs::set_permissions(&files.socket, fs::Permissions::from_mode(0o600))
        .map_err(StartError::io(&files.socket))?;
    let uid = fs::metadata(&files.socket)
        .map_err(StartError::io(&files.socket))?
        .uid();
    Ok((listener, uid))
}

/// Tells systemd about the daemon (`READY=1`, `STOPPING=1`), when it runs
/// under systemd with `Type=notify`. Otherwise does nothing.
pub fn notify_systemd(state: &str) {
    let Some(target) = std::env::var_os("NOTIFY_SOCKET") else {
        return;
    };
    if let Err(e) = send_notify(&target, state) {
        debug!(error = %e, "couldn't notify systemd");
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
        let (_listener, uid) = bind(&files, &lock).unwrap();
        assert_eq!(mode(&files.socket), 0o600);
        assert_eq!(uid, fs::metadata(root.path()).unwrap().uid());

        drop(lock);
        assert!(
            take_place(&files).is_ok(),
            "free again once the first is gone"
        );
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn systemd_is_told_over_its_socket() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("notify");
        let listener = std::os::unix::net::UnixDatagram::bind(&path).unwrap();
        send_notify(path.as_os_str(), "READY=1").unwrap();
        let mut buffer = [0_u8; 32];
        let n = listener.recv(&mut buffer).unwrap();
        assert_eq!(&buffer[..n], b"READY=1");
    }
}
