//! `priya doctor` against real parts in temporary folders (0.7).

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use common::{Machine, machine};
use priya_api::methods::system::{
    CheckResult, CheckStatus, DoctorReport, FixInfo, FixMode, FixScope,
};
use priya_testkit::assert_golden_json;
use priyad::doctor::{self, Context};

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

/// What doctor runs with on the temporary machine.
fn context(m: &Machine) -> Context {
    let paths = m.daemon.doctor.clone();
    Context {
        daemon: m.daemon.clone(),
        runtime: paths.runtime,
        data_dir: paths.data_dir,
        cli_version: Some(env!("CARGO_PKG_VERSION").into()),
        mountinfo: paths.mountinfo,
        fstab: paths.fstab,
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
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
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
    let context = context(&m);
    let socket = &context.runtime.socket;
    fs::set_permissions(socket, fs::Permissions::from_mode(0o666)).unwrap();
    let mode = || fs::metadata(socket).unwrap().permissions().mode() & 0o777;

    let dry = doctor::run(&context, &doctor::checks(), Some(FixMode::DryRun)).await;
    let check = status(&dry, "socket.permissions");
    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(check.fix.as_ref().unwrap().scope, FixScope::User);
    assert!(dry.fixed.is_empty());
    assert_eq!(mode(), 0o666, "a dry run changes nothing");

    let applied = doctor::run(&context, &doctor::checks(), Some(FixMode::Apply)).await;
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
    let m = machine().await;
    let mut context = context(&m);
    fs::create_dir_all(m.user_settings.parent().unwrap()).unwrap();
    fs::write(&m.user_settings, "[ui]\ntheme = \"darkk\"\n").unwrap();
    context.cli_version = Some("0.0.0-old".into());
    let report = doctor::run(&context, &doctor::checks(), None).await;
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

/// The fixture disk `name` from priya-core's mountinfo fixtures.
fn on_disk(context: &mut Context, name: &str) {
    let disk = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../priya-core/tests/fixtures/mountinfo")
        .join(name);
    context.mountinfo = disk.join("mountinfo");
    context.fstab = disk.join("fstab");
}

// covers: CORE-05
// suite: integration
#[tokio::test]
async fn other_disks_say_what_priya_sandbox_lacks() {
    let m = machine().await;
    let mut context = context(&m);

    on_disk(&mut context, "ext4");
    let report = doctor::run(&context, &doctor::checks(), None).await;
    assert_eq!(
        status(&report, "fs.root_type").summary,
        "The system disk uses ext4."
    );
    let layout = status(&report, "fs.btrfs_layout");
    assert_eq!(layout.status, CheckStatus::Info);
    assert!(
        layout
            .details
            .as_deref()
            .unwrap()
            .contains("The system disk uses ext4, not btrfs."),
        "{layout:?}"
    );

    on_disk(&mut context, "btrfs-plain");
    let report = doctor::run(&context, &doctor::checks(), None).await;
    let details = status(&report, "fs.btrfs_layout").details.clone().unwrap();
    assert!(details.contains("Missing subvolumes"), "{details}");

    // No mountinfo at all: not Linux.
    context.mountinfo = context.data_dir.join("no-mountinfo");
    let report = doctor::run(&context, &doctor::checks(), None).await;
    for id in ["fs.root_type", "fs.btrfs_layout"] {
        assert_eq!(
            status(&report, id).summary,
            "This isn't a Linux system, so there's nothing to check."
        );
    }
}

// covers: CORE-05
// suite: integration
#[tokio::test]
async fn a_changed_history_and_an_unknown_cli_are_reported() {
    let m = machine().await;
    let mut context = context(&m);
    context.cli_version = None;
    m.daemon
        .settings
        .set(
            &priya_core::settings::KeyPath::parse("ui.theme").unwrap(),
            &serde_json::json!("light"),
        )
        .await
        .unwrap();
    let db = rusqlite::Connection::open(&m.daemon.db_path).unwrap();
    db.execute_batch(
        "DROP TRIGGER IF EXISTS events_no_update;
         UPDATE events SET payload = json_set(payload, '$.new', 'dark') WHERE seq = 1;",
    )
    .unwrap();

    let report = doctor::run(&context, &doctor::checks(), None).await;
    let store = status(&report, "store.integrity");
    assert_eq!(store.status, CheckStatus::Fail);
    assert_eq!(
        store.summary,
        "Something in the history was changed, removed or reordered."
    );
    assert_eq!(
        status(&report, "daemon.version_matches_cli").status,
        CheckStatus::Info
    );
}

// covers: CORE-05
// suite: integration
#[tokio::test]
async fn a_fix_that_fails_says_so() {
    let m = machine().await;
    let context = context(&m);
    // A socket that is gone can be neither checked nor fixed.
    fs::remove_file(&context.runtime.socket).unwrap();
    let report = doctor::run(&context, &doctor::checks(), Some(FixMode::Apply)).await;
    let socket = status(&report, "socket.permissions");
    assert_eq!(socket.status, CheckStatus::Fail, "{socket:?}");
    assert!(report.fixed.is_empty());
    assert!(
        socket
            .details
            .as_deref()
            .unwrap()
            .contains("The fix didn't work"),
        "{socket:?}"
    );
}
