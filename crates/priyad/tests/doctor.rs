//! `priya doctor` against real parts in temporary folders (0.7).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use priya_api::methods::system::{
    CheckResult, CheckStatus, DoctorReport, FixInfo, FixMode, FixScope,
};
use priya_core::clock::SystemClock;
use priya_store::{EventStore, StoreConfig};
use priya_testkit::assert_golden_json;
use priyad::api::{Daemon, DoctorPaths};
use priyad::daemon::{RuntimeFiles, bind, take_place};
use priyad::doctor::{self, Context};
use priyad::settings::{SettingsDirs, SettingsService};
use tempfile::TempDir;

// covers: CORE-05
// suite: unit
// acceptance: 0.7
#[test]
fn doctor_results_serialize_stably() {
    let report = DoctorReport {
        checks: vec![
            CheckResult {
                id: "store.integrity".into(),
                title: "The history of what happened is intact".into(),
                status: CheckStatus::Ok,
                summary: "All 42 events check out.".into(),
                details: None,
                fix: None,
            },
            CheckResult {
                id: "socket.permissions".into(),
                title: "Only you can talk to the background service".into(),
                status: CheckStatus::Fail,
                summary: "Other users might be able to talk to the background service.".into(),
                details: Some("/run/user/1000/priya/priyad.sock has mode 666, not 600".into()),
                fix: Some(FixInfo {
                    scope: FixScope::User,
                    description:
                        "Make the background service's folder and socket private to you again."
                            .into(),
                }),
            },
        ],
        fixed: vec!["socket.permissions".into()],
    };
    assert_golden_json!("doctor_report", report);
}

struct Machine {
    _root: TempDir,
    context: Context,
    _lock: priyad::daemon::InstanceLock,
    _listener: tokio::net::UnixListener,
    user_settings: PathBuf,
}

/// A daemon's parts in temporary folders, on the PriyaOS fixture disk.
async fn machine() -> Machine {
    let root = tempfile::tempdir().expect("a temporary folder");
    let base = root.path();
    let runtime = RuntimeFiles::in_dir(&base.join("run/priya"));
    let lock = take_place(&runtime).expect("the lock");
    let (listener, _) = bind(&runtime, &lock).expect("the socket");
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
    let paths = DoctorPaths {
        runtime: runtime.clone(),
        data_dir: base.join("data"),
        mountinfo: disk.join("mountinfo"),
        fstab: disk.join("fstab"),
    };
    let daemon = Arc::new(Daemon {
        store,
        settings,
        db_path: base.join("data/priya.db"),
        started: Instant::now(),
        doctor: paths.clone(),
    });
    let context = Context {
        daemon,
        runtime: paths.runtime,
        data_dir: paths.data_dir,
        cli_version: Some(env!("CARGO_PKG_VERSION").into()),
        mountinfo: paths.mountinfo,
        fstab: paths.fstab,
    };
    Machine {
        _root: root,
        context,
        _lock: lock,
        _listener: listener,
        user_settings,
    }
}

fn status<'a>(report: &'a DoctorReport, id: &str) -> &'a CheckResult {
    report
        .checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no check {id}"))
}

// covers: CORE-05
// suite: integration
#[tokio::test]
async fn a_healthy_machine_passes_every_check() {
    let m = machine().await;
    let report = doctor::run(&m.context, &doctor::checks(), None).await;
    let ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "daemon.version_matches_cli",
            "socket.permissions",
            "store.integrity",
            "settings.valid",
            "disk.free_space",
            "fs.root_type",
            "fs.btrfs_layout",
        ]
    );
    for check in &report.checks {
        assert!(
            matches!(
                check.status,
                CheckStatus::Ok | CheckStatus::Info | CheckStatus::Warn
            ),
            "{check:?}"
        );
    }
    assert_eq!(
        status(&report, "fs.root_type").summary,
        "The system disk uses btrfs."
    );
    assert_eq!(
        status(&report, "fs.btrfs_layout").summary,
        "The disk has the PriyaOS layout."
    );
}

// covers: CORE-05
// suite: integration
// acceptance: 0.7
#[tokio::test]
async fn doctor_fix_dry_run_changes_nothing() {
    let m = machine().await;
    let socket = &m.context.runtime.socket;
    fs::set_permissions(socket, fs::Permissions::from_mode(0o666)).unwrap();
    let mode = || fs::metadata(socket).unwrap().permissions().mode() & 0o777;

    let dry = doctor::run(&m.context, &doctor::checks(), Some(FixMode::DryRun)).await;
    let check = status(&dry, "socket.permissions");
    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(check.fix.as_ref().unwrap().scope, FixScope::User);
    assert!(dry.fixed.is_empty());
    assert_eq!(mode(), 0o666, "a dry run changes nothing");

    let applied = doctor::run(&m.context, &doctor::checks(), Some(FixMode::Apply)).await;
    assert_eq!(applied.fixed, ["socket.permissions"]);
    assert_eq!(
        status(&applied, "socket.permissions").status,
        CheckStatus::Ok
    );
    assert_eq!(mode(), 0o600);
}

// covers: CORE-05
// suite: integration
#[tokio::test]
async fn broken_settings_and_a_mismatched_cli_are_reported() {
    let mut m = machine().await;
    fs::create_dir_all(m.user_settings.parent().unwrap()).unwrap();
    fs::write(&m.user_settings, "[ui]\ntheme = \"darkk\"\n").unwrap();
    m.context.cli_version = Some("0.0.0-old".into());
    let report = doctor::run(&m.context, &doctor::checks(), None).await;
    let settings = status(&report, "settings.valid");
    assert_eq!(settings.status, CheckStatus::Fail);
    assert!(
        settings
            .details
            .as_deref()
            .unwrap()
            .contains("must be light or dark")
    );
    assert_eq!(
        status(&report, "daemon.version_matches_cli").status,
        CheckStatus::Warn
    );
}
