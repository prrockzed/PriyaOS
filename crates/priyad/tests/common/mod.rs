//! A daemon's parts in temporary folders, shared by the integration tests.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use priya_core::clock::SystemClock;
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
    _root: TempDir,
    _lock: InstanceLock,
    /// The daemon's socket, bound but not yet served.
    #[allow(dead_code)] // Only the tests that serve it read it.
    pub listener: Option<UnixListener>,
    pub daemon: Arc<Daemon>,
    /// The user's settings file, which may not exist yet.
    #[allow(dead_code)] // Only the settings tests read it.
    pub user_settings: PathBuf,
}

/// A daemon's parts in temporary folders, on the PriyaOS fixture disk.
pub async fn machine() -> Machine {
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
    let settings = SettingsService::start(dirs, store.clone()).await;
    let disk = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../priya-core/tests/fixtures/mountinfo/priyaos");
    let daemon = Daemon::new(
        DaemonParts {
            store,
            settings,
            db_path: base.join("data/priya.db"),
            started: Instant::now(),
            doctor: DoctorPaths {
                runtime,
                data_dir: base.join("data"),
                mountinfo: disk.join("mountinfo"),
                fstab: disk.join("fstab"),
            },
        },
        HostConfig {
            dirs: vec![base.join("modules")],
            builtins: Builtins::new(),
            timings: Timings::default(),
            policy: RestartPolicy::default(),
            spawn: Spawn::Direct,
        },
    );
    Machine {
        _root: root,
        _lock: lock,
        listener: Some(listener),
        daemon,
        user_settings,
    }
}
