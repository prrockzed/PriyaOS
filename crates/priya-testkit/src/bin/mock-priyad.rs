//! `mock-priyad --socket <path> --script <file> [--log <file>]`: plays a
//! script as priyad would on its socket, for UI tests (see
//! `priya_testkit::mock_priyad`). `--log` appends every line clients send.
//! It runs until SIGTERM or Ctrl+C, then removes its socket.

#![allow(clippy::print_stderr)] // a test tool reporting a bad start

use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use priya_testkit::mock_priyad::{Script, serve_logged};
use tokio::signal::unix::{SignalKind, signal};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let usage = || {
        eprintln!("usage: mock-priyad --socket <path> --script <file> [--log <file>]");
        ExitCode::from(2)
    };
    let (mut socket, mut script, mut log) = (None, None, None);
    while let Some(arg) = args.next() {
        let wanted = match arg.as_str() {
            "--socket" => &mut socket,
            "--script" => &mut script,
            "--log" => &mut log,
            other => {
                eprintln!("mock-priyad: unknown argument {other}");
                return ExitCode::from(2);
            }
        };
        // A flag without its value is a mistake, not "leave it out".
        let Some(value) = args.next() else {
            return usage();
        };
        *wanted = Some(PathBuf::from(value));
    }
    let (Some(socket), Some(script)) = (socket, script) else {
        return usage();
    };
    let script = match Script::read(&script) {
        Ok(script) => script,
        Err(e) => {
            eprintln!("mock-priyad: {e}");
            return ExitCode::FAILURE;
        }
    };
    let log = match log.map(|path| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
    }) {
        None => None,
        Some(Ok(file)) => Some(file),
        Some(Err(e)) => {
            eprintln!("mock-priyad: {e}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("mock-priyad: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(&socket, script, log)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mock-priyad: {}: {e}", socket.display());
            ExitCode::FAILURE
        }
    }
}

async fn run(socket: &Path, script: Script, log: Option<std::fs::File>) -> std::io::Result<()> {
    if let Some(dir) = socket.parent() {
        std::fs::create_dir_all(dir)?;
    }
    clear(socket)?;
    let listener = tokio::net::UnixListener::bind(socket)?;
    // Like priyad's: only its user may connect.
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    let mut term = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    let served = tokio::select! {
        served = serve_logged(listener, script, log) => served,
        _ = term.recv() => Ok(()),
        _ = interrupt.recv() => Ok(()),
    };
    let _ = std::fs::remove_file(socket);
    served
}

/// Removes a stale socket at `path`, but never a live one (a real priyad's)
/// or anything that isn't a socket.
fn clear(path: &Path) -> std::io::Result<()> {
    let Ok(found) = std::fs::symlink_metadata(path) else {
        return Ok(());
    };
    if !found.file_type().is_socket() {
        return Err(std::io::Error::other(
            "something that isn't a socket is in the way",
        ));
    }
    if std::os::unix::net::UnixStream::connect(path).is_ok() {
        return Err(std::io::Error::other(
            "something is already listening there",
        ));
    }
    std::fs::remove_file(path)
}
