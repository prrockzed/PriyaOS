//! The priyad binary: the per-user PriyaOS daemon (02-core).

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use priya_api::events::{Actor, DaemonStarted, DaemonStopped, NewEvent};
use priya_core::clock::SystemClock;
use priya_core::paths::PriyaPaths;
use priya_store::{EventStore, StoreConfig};
use priyad::api::{Daemon, router};
use priyad::daemon::{RuntimeFiles, bind, notify_systemd, take_place};
use priyad::rpc::server::serve;
use priyad::settings::{DEBOUNCE, SettingsDirs, SettingsService, watch};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::watch as channel;
use tracing::{error, info, warn};

/// The PriyaOS daemon. systemd starts it (`priyad.service`); it never forks.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// Run in the foreground. priyad always does; accepted for scripts and
    /// tests that say so.
    #[arg(long)]
    foreground: bool,
}

fn main() -> ExitCode {
    let _ = Args::parse();
    priyad::logging::init("info");
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            error!(error = %e, "couldn't start");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            // Logs go to stderr outside journald, so this reaches the
            // terminal too.
            error!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let started = Instant::now();
    let paths = PriyaPaths::from_env().map_err(|e| e.to_string())?;
    let files = RuntimeFiles::in_dir(&paths.runtime);
    let lock = take_place(&files).map_err(|e| e.to_string())?;

    let store_config = StoreConfig::in_data_dir(&paths.data);
    let store =
        EventStore::open(&store_config, Arc::new(SystemClock)).map_err(|e| e.to_string())?;
    let settings = SettingsService::start(settings_dirs(&paths), store.clone()).await;
    let _watcher = watch(settings.clone(), DEBOUNCE)
        .inspect_err(|e| warn!(error = %e, "settings won't reload live"))
        .ok();
    record(
        &store,
        &DaemonStarted {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            pid: std::process::id(),
        },
    )
    .await;

    let (listener, uid) = bind(&files, &lock).map_err(|e| e.to_string())?;
    let (stop, stopping) = channel::channel(false);
    tokio::spawn(async move {
        wait_for_signal().await;
        let _ = stop.send(true);
    });
    let daemon = Arc::new(Daemon {
        store: store.clone(),
        settings,
        db_path: store_config.db.clone(),
        started,
    });
    notify_systemd("READY=1");
    info!(socket = %files.socket.display(), "priyad is ready");

    serve(listener, uid, Arc::new(router()), daemon, stopping).await;

    notify_systemd("STOPPING=1");
    info!("priyad is stopping");
    record(
        &store,
        &DaemonStopped {
            reason: "asked to stop".into(),
        },
    )
    .await;
    let _ = std::fs::remove_file(&files.socket);
    // Dropping the last store handle waits for the writer to finish.
    drop(store);
    Ok(())
}

/// The settings folders. `PRIYA_DEFAULTS_DIR` and `PRIYA_SYSTEM_DIR`
/// override the installed ones, for tests and development.
fn settings_dirs(paths: &PriyaPaths) -> SettingsDirs {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs = SettingsDirs::installed(paths, home);
    if let Some(dir) = std::env::var_os("PRIYA_DEFAULTS_DIR") {
        dirs.defaults = dir.into();
    }
    if let Some(dir) = std::env::var_os("PRIYA_SYSTEM_DIR") {
        dirs.system = dir.into();
    }
    dirs
}

async fn record<P: priya_api::events::EventPayload>(store: &EventStore, payload: &P) {
    match NewEvent::new("daemon", Actor::System, payload) {
        Ok(event) => {
            if let Err(e) = store.append(event).await {
                warn!(error = %e, "couldn't record a daemon event");
            }
        }
        Err(e) => warn!(error = %e, "couldn't build a daemon event"),
    }
}

/// Resolves on SIGTERM (systemd) or SIGINT (Ctrl+C).
async fn wait_for_signal() {
    let mut term = signal(SignalKind::terminate()).ok();
    let mut int = signal(SignalKind::interrupt()).ok();
    tokio::select! {
        _ = async { if let Some(term) = term.as_mut() { term.recv().await; } else { std::future::pending::<()>().await } } => {}
        _ = async { if let Some(int) = int.as_mut() { int.recv().await; } else { std::future::pending::<()>().await } } => {}
    }
}
