//! A daemon's parts in temporary folders, shared by the integration tests.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use std::sync::Mutex;

use priya_ai::keys::{KeyError, MemorySecretStore, SecretStore};
use priya_ai::provider::BoxFuture;
use priya_core::clock::SystemClock;
use priya_core::secret::Secret;
use priya_store::{EventStore, StoreConfig};
use priyad::api::{Daemon, DaemonParts, DoctorPaths};
use priyad::daemon::{InstanceLock, RuntimeFiles, bind, take_place};
use priyad::modules::launch::{Builtins, Spawn};
use priyad::modules::policy::RestartPolicy;
use priyad::modules::{HostConfig, Timings};
use priyad::settings::{SettingsDirs, SettingsService};
use tempfile::TempDir;
use tokio::net::UnixListener;

pub struct Machine {
    root: TempDir,
    /// The keys priyad reads, in memory.
    #[allow(dead_code)] // Only the tests with keys use it.
    pub keys: Arc<TestKeyring>,
    _lock: InstanceLock,
    /// The daemon's socket, bound but not yet served.
    #[allow(dead_code)] // Only the tests that serve it read it.
    pub listener: Option<UnixListener>,
    pub daemon: Arc<Daemon>,
    /// The user's settings file, which may not exist yet.
    #[allow(dead_code)] // Not every test file reads it.
    pub user_settings: PathBuf,
    /// Where the daemon looks for modules.
    #[allow(dead_code)] // Only the module tests read it.
    pub modules: PathBuf,
    /// What the desktop checks hear from `qs`, `hyprctl` and the like.
    #[allow(dead_code)] // Only the doctor tests set it up.
    pub desktop: Arc<Desktop>,
}

/// A daemon's parts in temporary folders, on the PriyaOS fixture disk.
#[allow(dead_code)] // The module tests use `machine_with`.
pub async fn machine() -> Machine {
    machine_with(Builtins::new(), Timings::default()).await
}

/// [`machine`] with these builtin modules and module timings.
pub async fn machine_with(builtins: Builtins, timings: Timings) -> Machine {
    let root = tempfile::tempdir().expect("a temporary folder");
    let base = root.path();
    let runtime = RuntimeFiles::in_dir(&base.join("run/priya"));
    let lock = take_place(&runtime).expect("the lock");
    let listener = bind(&runtime, &lock).expect("the socket");
    let store = EventStore::open(
        &StoreConfig::in_data_dir(&base.join("data")),
        Arc::new(SystemClock),
    )
    .expect("the store opens");
    let dirs = SettingsDirs {
        defaults: base.join("defaults"),
        system: base.join("etc"),
        user: base.join("config"),
        home: Some(base.to_path_buf()),
    };
    let user_settings = dirs.user_file();
    let modules = base.join("modules");
    let settings = SettingsService::start(dirs, store.clone()).await;
    let disk = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../priya-core/tests/fixtures/mountinfo/priyaos");
    let keys = Arc::new(TestKeyring::default());
    let ai = priyad::ai::answerer(
        keys.clone(),
        store.clone(),
        Arc::new(SystemClock),
        priya_ai::gateway::prompt::BUILT_IN.to_owned(),
        time::UtcOffset::UTC,
    )
    .expect("an HTTP client");
    let desktop = Arc::new(Desktop::default());
    let daemon = Daemon::new(
        DaemonParts {
            store,
            ai,
            settings,
            db_path: base.join("data/priya.db"),
            started: Instant::now(),
            doctor: DoctorPaths {
                runtime,
                data_dir: base.join("data"),
                mountinfo: disk.join("mountinfo"),
                fstab: disk.join("fstab"),
                commands: desktop.clone(),
            },
        },
        HostConfig {
            dirs: vec![modules.clone()],
            builtins,
            timings,
            policy: RestartPolicy::default(),
            spawn: Spawn::Direct,
        },
    );
    Machine {
        root,
        keys,
        _lock: lock,
        listener: Some(listener),
        daemon,
        user_settings,
        modules,
        desktop,
    }
}

/// The desktop a test machine has: canned answers by command line, so the
/// desktop doctor checks don't ask the real computer. Anything not set up
/// can't be run.
#[derive(Debug, Default)]
pub struct Desktop {
    pub answers: std::sync::Mutex<std::collections::HashMap<String, (bool, String)>>,
}

impl Desktop {
    #[allow(dead_code)] // Only the doctor tests set it up.
    pub fn answer(&self, command: &str, success: bool, stdout: &str) {
        self.answers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(command.into(), (success, stdout.into()));
    }
}

impl priyad::desktop::Commands for Desktop {
    fn run<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> priyad::doctor::BoxFuture<'a, priyad::desktop::Ran> {
        let line = std::iter::once(program)
            .chain(args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        let answer = self
            .answers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&line)
            .cloned();
        Box::pin(async move {
            answer
                .map(|(success, stdout)| priyad::desktop::Output { success, stdout })
                .ok_or(priyad::desktop::RunError::NotFound)
        })
    }
}

impl Machine {
    /// Where all of this machine's files are.
    #[allow(dead_code)] // Only the security tests search it.
    pub fn root(&self) -> &Path {
        self.root.path()
    }
}

/// Keys in memory, or a keyring that fails as told (locked, missing...).
#[derive(Default)]
pub struct TestKeyring {
    keys: MemorySecretStore,
    failure: Mutex<Option<KeyError>>,
}

impl TestKeyring {
    /// From now on every use fails with `failure` (`None`: works again).
    #[allow(dead_code)] // Only the doctor tests break it.
    pub fn fail_with(&self, failure: Option<KeyError>) {
        *self.failure.lock().unwrap_or_else(|e| e.into_inner()) = failure;
    }

    fn failure(&self) -> Option<KeyError> {
        self.failure
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl SecretStore for TestKeyring {
    fn get(&self, provider: &str) -> BoxFuture<'_, Result<Option<Secret<String>>, KeyError>> {
        match self.failure() {
            Some(e) => Box::pin(async move { Err(e) }),
            None => self.keys.get(provider),
        }
    }

    fn set(&self, provider: &str, key: Secret<String>) -> BoxFuture<'_, Result<(), KeyError>> {
        match self.failure() {
            Some(e) => Box::pin(async move { Err(e) }),
            None => self.keys.set(provider, key),
        }
    }

    fn remove(&self, provider: &str) -> BoxFuture<'_, Result<bool, KeyError>> {
        match self.failure() {
            Some(e) => Box::pin(async move { Err(e) }),
            None => self.keys.remove(provider),
        }
    }
}
