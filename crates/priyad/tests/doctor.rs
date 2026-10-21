//! `priya doctor` against real parts in temporary folders (0.7).

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use common::{Machine, machine};
use priya_ai::keys::KeyError;
use priya_api::methods::system::{
    CheckResult, CheckStatus, DoctorReport, FixInfo, FixMode, FixScope,
};
use priya_testkit::assert_golden_json;
use priyad::doctor::{self, Context};

// covers: CORE-05
// suite: unit
// acceptance: 0.7
// regression: 0.7
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
            "ai.configured",
            "ai.key_present",
            "ai.provider_reachable",
            "secret_service.available",
            "install.paths",
            "systemd.user_units",
            "desktop.quickshell_installed",
            "desktop.priya_shell_running",
            "desktop.shortcut_free",
            "desktop.hyprland_snippet_sourced",
            "desktop.fonts_present",
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
    assert_eq!(
        status(&report, "desktop.hyprland_snippet_sourced").status,
        CheckStatus::Info,
        "a test machine has no Hyprland"
    );
}

// covers: CORE-05
// suite: integration
// acceptance: 0.7
// regression: 0.7
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

/// Sets one setting on the test machine.
async fn set(m: &Machine, key: &str, value: serde_json::Value) {
    let key = priya_core::settings::KeyPath::parse(key).expect("a key");
    m.daemon
        .settings
        .set(&key, &value)
        .await
        .expect("the setting is taken");
}

// covers: AI-21, CORE-05
// suite: integration
// acceptance: 0.12
#[tokio::test]
async fn ai_checks_follow_the_model_setup() {
    let m = machine().await;
    let ai_checks = |report: &DoctorReport| -> Vec<(String, CheckStatus)> {
        report
            .checks
            .iter()
            .filter(|c| c.id.starts_with("ai.") || c.id.starts_with("secret_service."))
            .map(|c| (c.id.clone(), c.status))
            .collect()
    };
    let context = context(&m);
    let checks = doctor::checks();
    let run = || doctor::run(&context, &checks, None);

    // A fresh install: nothing chosen, nothing broken.
    let fresh = run().await;
    assert!(
        ai_checks(&fresh)
            .iter()
            .all(|(_, s)| matches!(s, CheckStatus::Info | CheckStatus::Ok)),
        "{fresh:?}"
    );
    assert!(
        status(&fresh, "ai.configured")
            .summary
            .contains("priya ai key set gemini")
    );

    // A model on this computer that answers.
    let server = priya_testkit::http_script::ScriptedHttp::start(vec![
        priya_testkit::http_script::HttpReply::status(200, r#"{"data":[]}"#),
    ])
    .await
    .unwrap();
    set(
        &m,
        "ai.providers.local",
        serde_json::json!({"kind": "openai_compatible", "base_url": server.base_url()}),
    )
    .await;
    set(
        &m,
        "ai.strategy",
        serde_json::json!("fixed:local/qwen2.5:0.5b"),
    )
    .await;
    let local = run().await;
    assert_eq!(
        ai_checks(&local),
        [
            ("ai.configured".to_owned(), CheckStatus::Ok),
            ("ai.key_present".to_owned(), CheckStatus::Ok),
            ("ai.provider_reachable".to_owned(), CheckStatus::Ok),
            ("secret_service.available".to_owned(), CheckStatus::Ok),
        ]
    );
    assert_eq!(
        status(&local, "ai.configured").summary,
        "qwen2.5:0.5b answers, from the local model server."
    );
    let asked: Vec<_> = server.requests().iter().map(|r| r.path.clone()).collect();
    assert_eq!(asked, ["/v1/models"], "one free call, no tokens spent");

    // No keyring: fine for a model on this computer.
    m.keys
        .fail_with(Some(KeyError::Unavailable("no D-Bus".into())));
    let no_keyring = run().await;
    assert_eq!(
        status(&no_keyring, "secret_service.available").status,
        CheckStatus::Info
    );
    assert_eq!(
        status(&no_keyring, "ai.key_present").status,
        CheckStatus::Ok
    );
    m.keys.fail_with(None);

    // A cloud model without its key: a warning that says what to run.
    set(
        &m,
        "ai.providers.gemini",
        serde_json::json!({"kind": "openai_compatible", "base_url": "https://example.invalid/v1"}),
    )
    .await;
    set(
        &m,
        "ai.strategy",
        serde_json::json!("fixed:gemini/gemini-2.5-flash"),
    )
    .await;
    let cloud = run().await;
    let key = status(&cloud, "ai.key_present");
    assert_eq!(key.status, CheckStatus::Warn);
    assert!(key.summary.contains("priya ai key set gemini"), "{key:?}");
    assert_eq!(
        status(&cloud, "ai.provider_reachable").status,
        CheckStatus::Warn
    );

    // A locked keyring: warnings that say how to unlock it.
    m.keys.fail_with(Some(KeyError::Locked));
    let locked = run().await;
    for id in ["ai.key_present", "secret_service.available"] {
        let check = status(&locked, id);
        assert_eq!(check.status, CheckStatus::Warn, "{check:?}");
        assert!(check.summary.contains("Unlock it"), "{check:?}");
    }
    m.keys.fail_with(None);

    // A provider that isn't set up: the setup fails, the others wait on it.
    set(
        &m,
        "ai.strategy",
        serde_json::json!("fixed:nowhere/a-model"),
    )
    .await;
    let broken = run().await;
    assert_eq!(status(&broken, "ai.configured").status, CheckStatus::Fail);
    for id in ["ai.key_present", "ai.provider_reachable"] {
        let check = status(&broken, id);
        assert_eq!(check.status, CheckStatus::Info, "{check:?}");
        assert!(check.summary.contains("needs fixing first"), "{check:?}");
    }
}

// covers: UX-02, CORE-05
// suite: integration
#[tokio::test]
async fn desktop_checks_say_what_the_bar_needs() {
    let m = machine().await;
    // Nothing answers: no desktop here, which isn't a failure.
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    assert_eq!(
        status(&report, "desktop.quickshell_installed").status,
        CheckStatus::Warn
    );
    for id in [
        "desktop.priya_shell_running",
        "desktop.shortcut_free",
        "desktop.fonts_present",
    ] {
        assert_eq!(status(&report, id).status, CheckStatus::Info, "{id}");
    }

    // A desktop with every problem the bar can have.
    m.desktop.answer("qs --version", true, "quickshell 0.1.4\n");
    m.desktop.answer(
        "systemctl --user is-active priya-shell",
        false,
        "inactive\n",
    );
    m.desktop.answer(
        "hyprctl binds -j",
        true,
        r#"[{"modmask": 64, "key": "SPACE", "dispatcher": "exec", "arg": "rofi -show drun"}]"#,
    );
    m.desktop.answer(
        "fc-list : family",
        true,
        "JetBrains Mono\nNoto Sans Devanagari,Noto Sans\n",
    );
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    let warned = |id| {
        let check = status(&report, id);
        assert_eq!(check.status, CheckStatus::Warn, "{check:?}");
        check
    };
    assert!(
        warned("desktop.quickshell_installed")
            .summary
            .contains("too old")
    );
    assert!(
        warned("desktop.priya_shell_running")
            .summary
            .contains("systemctl --user start priya-shell")
    );
    assert_eq!(
        warned("desktop.shortcut_free").details.as_deref(),
        Some("Super+Space runs: exec rofi -show drun")
    );
    assert_eq!(
        warned("desktop.fonts_present").summary,
        "Missing: Inter. Install with `sudo pacman -S inter-font`."
    );

    // And one with everything in place.
    m.desktop.answer("qs --version", true, "quickshell 0.2.1\n");
    m.desktop
        .answer("systemctl --user is-active priya-shell", true, "active\n");
    m.desktop.answer(
        "hyprctl binds -j",
        true,
        r#"[{"modmask": 64, "key": "space", "dispatcher": "exec", "arg": "qs -p /usr/share/priya/shell ipc call priya-bar toggle"}]"#,
    );
    m.desktop.answer(
        "fc-list : family",
        true,
        "Inter\nJetBrains Mono\nNoto Sans Devanagari\n",
    );
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    for id in [
        "desktop.quickshell_installed",
        "desktop.priya_shell_running",
        "desktop.shortcut_free",
        "desktop.fonts_present",
    ] {
        assert_eq!(status(&report, id).status, CheckStatus::Ok, "{id}");
    }
}

// covers: UX-02, CORE-05
// suite: integration
#[tokio::test]
async fn hyprland_is_checked_for_priyaos_lines() {
    let m = machine().await;
    let hypr = m.daemon.doctor.hypr_config.clone();
    let check = "desktop.hyprland_snippet_sourced";
    let found = |report: &DoctorReport| status(report, check).status;
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    assert_eq!(found(&report), CheckStatus::Info, "no config");

    // A Lua config (Hyprland 0.55 and later): comments and strings that
    // name the file don't load it.
    std::fs::create_dir_all(&hypr).unwrap();
    let lua = hypr.join("hyprland.lua");
    let line = r#"pcall(require, "/usr/share/priya/desktop/hyprland.lua")"#;
    for missing in [
        format!("-- {line}\n"),
        format!("hl.bind(\"SUPER + Q\", hl.dsp.exec_cmd(\"kitty\")) -- {line}\n"),
        format!("--[[\n{line}\n]]\n"),
    ] {
        std::fs::write(&lua, &missing).unwrap();
        let report = doctor::run(&context(&m), &doctor::checks(), None).await;
        let check = status(&report, check);
        assert_eq!(check.status, CheckStatus::Warn, "{missing}");
        assert!(check.summary.contains(line), "{}", check.summary);
    }
    std::fs::write(
        &lua,
        format!("hl.bind(\"SUPER + Q\", hl.dsp.exec_cmd(\"kitty\"))\n{line}\n"),
    )
    .unwrap();
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    assert_eq!(found(&report), CheckStatus::Ok);

    // An older config split into files, two levels down.
    std::fs::remove_file(&lua).unwrap();
    std::fs::write(
        hypr.join("hyprland.conf"),
        "source = conf/keys.conf # keys\n",
    )
    .unwrap();
    std::fs::create_dir_all(hypr.join("conf/more")).unwrap();
    std::fs::write(
        hypr.join("conf/keys.conf"),
        format!("source = {}\n", hypr.join("conf/more/priya.conf").display()),
    )
    .unwrap();
    std::fs::write(
        hypr.join("conf/more/priya.conf"),
        "# PriyaOS\nsource = /usr/share/priya/desktop/hyprland.conf\n",
    )
    .unwrap();
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    assert_eq!(found(&report), CheckStatus::Ok);

    // Whatever the files say, a Super+Space that opens the bar is enough.
    std::fs::write(hypr.join("conf/more/priya.conf"), "").unwrap();
    m.desktop.answer(
        "hyprctl binds -j",
        true,
        r#"[{"modmask": 64, "key": "SPACE", "dispatcher": "exec", "arg": "qs -p /usr/share/priya/shell ipc call priya-bar toggle"}]"#,
    );
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    assert_eq!(found(&report), CheckStatus::Ok);
}

// covers: OS-01, CORE-05
// suite: integration
#[tokio::test]
async fn install_checks_find_missing_files_and_a_service_that_wont_start() {
    let m = machine().await;
    let root = m.daemon.doctor.install_root.clone();
    let check = |report: &DoctorReport, id: &str| status(report, id).clone();
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    let missing = check(&report, "install.paths");
    assert_eq!(missing.status, CheckStatus::Warn);
    assert!(
        missing.summary.contains("sudo pacman -S priya"),
        "{}",
        missing.summary
    );
    assert!(missing.details.unwrap().contains("/usr/bin/priya"));
    assert_eq!(
        check(&report, "systemd.user_units").status,
        CheckStatus::Info,
        "no user manager"
    );

    for file in priyad::install::CORE_FILES {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    assert!(
        check(&report, "install.paths")
            .summary
            .contains("priya-shell"),
        "the bar is missing"
    );
    for file in priyad::install::SHELL_FILES {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
    m.desktop
        .answer("systemctl --user is-enabled priyad", false, "disabled\n");
    m.desktop
        .answer("systemctl --user is-active priyad", true, "active\n");
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    assert_eq!(check(&report, "install.paths").status, CheckStatus::Ok);
    let units = check(&report, "systemd.user_units");
    assert_eq!(units.status, CheckStatus::Warn);
    assert!(units.summary.contains("systemctl --user enable priyad"));

    m.desktop
        .answer("systemctl --user is-enabled priyad", true, "enabled\n");
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    assert_eq!(check(&report, "systemd.user_units").status, CheckStatus::Ok);
    m.desktop
        .answer("systemctl --user is-active priyad", false, "inactive\n");
    let report = doctor::run(&context(&m), &doctor::checks(), None).await;
    assert_eq!(
        check(&report, "systemd.user_units").status,
        CheckStatus::Info,
        "started by hand"
    );
}
