//! The priyad binary: the per-user PriyaOS daemon (02-core).

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use priya_ai::keys::SecretServiceStore;
use priya_api::events::{Actor, DaemonStarted, DaemonStopped, NewEvent};
use priya_core::clock::SystemClock;
use priya_core::paths::PriyaPaths;
use priya_core::settings::KeyPath;
use priya_store::{EventStore, StoreConfig};
use priyad::api::{Daemon, DaemonParts, DoctorPaths, router};
use priyad::daemon::{RuntimeFiles, Systemd, bind, own_uid, take_place};
use priyad::modules::launch::{Builtins, Spawn};
use priyad::modules::{HostConfig, parse_timings};
use priyad::rpc::server::serve;
use priyad::settings::{DEBOUNCE, SettingsDirs, SettingsService, watch};
use tokio::signal::unix::{Signal, SignalKind, signal};
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
    priya_logging::init("info");
    // Linux can tell the time zone only while the program has one thread,
    // so read it before the runtime starts its workers.
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            error!(error = %e, "couldn't start");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(offset)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            // Logs go to stderr outside journald, so this reaches the
            // terminal too.
            error!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run(offset: time::UtcOffset) -> Result<(), String> {
    let started = Instant::now();
    let systemd = Systemd::from_env();
    let mut signals = Signals::install().map_err(|e| format!("can't watch for signals: {e}"))?;
    let paths = PriyaPaths::from_env().map_err(|e| e.to_string())?;
    let files = RuntimeFiles::in_dir(&paths.runtime);
    let lock = take_place(&files).map_err(|e| e.to_string())?;
    // The socket first: a daemon that can't be reached shouldn't record
    // that it started.
    let listener = bind(&files, &lock).map_err(|e| e.to_string())?;

    let store_config = StoreConfig::in_data_dir(&paths.data);
    let store =
        EventStore::open(&store_config, Arc::new(SystemClock)).map_err(|e| e.to_string())?;
    // Modules' settings schemas count from the first load, so a section
    // that breaks one is left out at start too.
    let modules_config = module_config(&paths);
    let schemas = module_schemas(&modules_config.dirs).await;
    let settings =
        SettingsService::start_with_module_schemas(settings_dirs(&paths), store.clone(), schemas)
            .await;
    let watcher = watch(settings.clone(), DEBOUNCE)
        .inspect_err(|e| warn!(error = %e, "settings won't reload live"))
        .ok();
    // Before the start is recorded: a daemon that can't answer stops here.
    let ai = priyad::ai::answerer(
        Arc::new(SecretServiceStore),
        store.clone(),
        Arc::new(SystemClock),
        priyad::ai::prompt(&prompt_file()),
        offset,
    )
    .map_err(|e| e.to_string())?;
    record(
        &store,
        &DaemonStarted {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            pid: std::process::id(),
        },
    )
    .await;

    let daemon = Daemon::new(
        DaemonParts {
            store: store.clone(),
            ai,
            settings,
            db_path: store_config.db.clone(),
            started,
            doctor: DoctorPaths {
                runtime: files.clone(),
                data_dir: paths.data.clone(),
                mountinfo: "/proc/self/mountinfo".into(),
                fstab: "/etc/fstab".into(),
                commands: Arc::new(priyad::desktop::System),
            },
        },
        modules_config,
    );
    let (stop, stopping) = channel::channel(false);
    tokio::spawn({
        let systemd = systemd.clone();
        let intents = daemon.intents.clone();
        async move {
            signals.next().await;
            systemd.notify("STOPPING=1");
            info!("priyad is stopping");
            // Before connections close, so their answers record that
            // priyad stopped them.
            intents.closing();
            let _ = stop.send(true);
            signals.next().await;
            warn!("asked again: stopping now");
            std::process::exit(1);
        }
    });
    let modules = start_modules(&daemon).await;
    systemd.notify("READY=1");
    info!(socket = %files.socket.display(), "priyad is ready");

    serve(
        listener,
        own_uid(),
        Arc::new(router()),
        daemon.clone(),
        stopping,
    )
    .await;

    // Stop in the reverse order of starting: modules, no more file
    // changes, the last event, the store, and the lock last of all.
    // Stopping the host first: nothing starts afterwards, so the watch
    // task has nothing left to do when it is stopped.
    // Answers still going stop now and record it, before the last event.
    daemon.intents.shutdown(priyad::rpc::server::DRAIN).await;
    daemon.modules.stop("priyad is stopping").await;
    modules.abort();
    drop(daemon);
    if let Some(watcher) = watcher {
        watcher.stop().await;
    }
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
    drop(lock);
    Ok(())
}

/// Where modules are found, how fast they must answer, and which are built
/// in. `PRIYA_SYSTEM_MODULES_DIR` replaces `/usr/lib/priya/modules` and
/// `PRIYA_MODULE_TIMINGS` shortens the timings, for tests.
fn module_config(paths: &PriyaPaths) -> HostConfig {
    let system = std::env::var_os("PRIYA_SYSTEM_MODULES_DIR")
        .map_or_else(|| PathBuf::from("/usr/lib/priya/modules"), PathBuf::from);
    let (timings, policy) = match std::env::var("PRIYA_MODULE_TIMINGS") {
        Ok(text) => parse_timings(&text).unwrap_or_else(|e| {
            warn!("PRIYA_MODULE_TIMINGS is ignored: {e}");
            parse_timings("").unwrap_or_default()
        }),
        Err(_) => parse_timings("").unwrap_or_default(),
    };
    HostConfig {
        dirs: vec![system, paths.data.join("modules")],
        builtins: builtins(),
        timings,
        policy,
        spawn: Spawn::detect(),
    }
}

/// The settings schemas of the modules installed now, by name.
async fn module_schemas(dirs: &[PathBuf]) -> std::collections::BTreeMap<String, serde_json::Value> {
    let dirs = dirs.to_vec();
    let found = tokio::task::spawn_blocking(move || priyad::modules::discovery::discover(&dirs))
        .await
        .unwrap_or_default();
    found
        .modules
        .into_iter()
        .filter_map(|(name, module)| Some((name, module.settings_schema?)))
        .collect()
}

/// The modules compiled into priyad. The echo example joins with the
/// `builtin-examples` feature.
fn builtins() -> Builtins {
    #[allow(unused_mut)] // Empty without the feature.
    let mut builtins = Builtins::new();
    #[cfg(feature = "builtin-examples")]
    builtins.insert("echo", priya_echo_example::BUILTIN);
    builtins
}

/// Starts the enabled modules and follows `modules.enabled` from then on.
/// The task holds the daemon weakly, so it never keeps it alive.
async fn start_modules(daemon: &Arc<Daemon>) -> tokio::task::JoinHandle<()> {
    let handle = daemon.settings.handle();
    let mut changes = handle.watch(KeyPath::from_parts(["modules", "enabled"]));
    daemon
        .modules
        .set_enabled(&handle.current().modules.enabled)
        .await;
    let daemon = Arc::downgrade(daemon);
    tokio::spawn(async move {
        while changes.changed().await.is_some() {
            let Some(daemon) = daemon.upgrade() else {
                break;
            };
            // The list as it is now, read under the same lock as enable and
            // disable, so an older list never overtakes a newer one.
            let _changing = daemon.module_changes.lock().await;
            let enabled = daemon.settings.handle().current().modules.enabled.clone();
            daemon.modules.set_enabled(&enabled).await;
        }
    })
}

/// The answer prompt. `PRIYA_PROMPTS_DIR` replaces
/// `/usr/share/priya/prompts`, for tests and development.
fn prompt_file() -> PathBuf {
    std::env::var_os("PRIYA_PROMPTS_DIR").map_or_else(
        || PathBuf::from(priyad::ai::PROMPT_FILE),
        |dir| PathBuf::from(dir).join("answer.md"),
    )
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

/// SIGTERM (from systemd) and SIGINT (Ctrl+C).
struct Signals {
    term: Signal,
    interrupt: Signal,
}

impl Signals {
    fn install() -> std::io::Result<Self> {
        Ok(Self {
            term: signal(SignalKind::terminate())?,
            interrupt: signal(SignalKind::interrupt())?,
        })
    }

    /// Resolves on the next of either.
    async fn next(&mut self) {
        tokio::select! {
            _ = self.term.recv() => {}
            _ = self.interrupt.recv() => {}
        }
    }
}
