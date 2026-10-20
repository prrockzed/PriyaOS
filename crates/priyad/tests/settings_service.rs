//! The settings service against real files and a real watcher (0.6).

use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use priya_api::events::NewEvent;
use priya_core::settings::{KeyPath, Theme};
use priya_testkit::LoopChild;
use priyad::settings::{DEBOUNCE, EventSink, SettingsDirs, SettingsService, watch, write_atomic};
use serde_json::{Value, json};
use tempfile::TempDir;

const SHIPPED: &str = include_str!("../../../data/defaults/00-core.toml");

/// Records events in memory.
#[derive(Clone, Default)]
struct Recorded(Arc<Mutex<Vec<NewEvent>>>);

impl EventSink for Recorded {
    async fn record(&self, events: Vec<NewEvent>) -> Result<(), String> {
        self.0.lock().unwrap().extend(events);
        Ok(())
    }
}

impl Recorded {
    /// The payloads of the events of `event_type`, in order.
    fn of_type(&self, event_type: &str) -> Vec<Value> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.event_type == event_type)
            .map(|e| e.payload.clone())
            .collect()
    }

    /// Waits up to `within` for `count` events of `event_type`.
    async fn wait_for(&self, event_type: &str, count: usize, within: Duration) -> Vec<Value> {
        let deadline = Instant::now() + within;
        loop {
            let found = self.of_type(event_type);
            if found.len() >= count || Instant::now() > deadline {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

struct Fixture {
    // Kept for its Drop, which deletes the folders.
    _root: TempDir,
    dirs: SettingsDirs,
}

impl Fixture {
    /// Shipped defaults installed; no system or user settings yet.
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        // Canonical, as the home folder is in practice (macOS's temp folder
        // is behind a symlink).
        let base = root.path().canonicalize().unwrap();
        let dirs = SettingsDirs {
            defaults: base.join("usr/share/priya/defaults"),
            system: base.join("etc/priya"),
            user: base.join("home/.config/priya"),
            home: Some(base.join("home")),
        };
        fs::create_dir_all(&dirs.defaults).unwrap();
        fs::write(dirs.defaults.join("00-core.toml"), SHIPPED).unwrap();
        Self { _root: root, dirs }
    }

    /// Writes the user's settings file the way an editor would.
    fn edit(&self, text: &str) {
        fs::create_dir_all(&self.dirs.user).unwrap();
        fs::write(self.dirs.user_file(), text).unwrap();
    }

    async fn start(&self) -> (Arc<SettingsService<Recorded>>, Recorded) {
        let recorded = Recorded::default();
        let service = SettingsService::start(self.dirs.clone(), recorded.clone()).await;
        (service, recorded)
    }
}

fn key(text: &str) -> KeyPath {
    KeyPath::parse(text).unwrap()
}

// covers: CORE-03
// suite: integration
// acceptance: 0.6
#[tokio::test]
async fn hot_reload_applies_valid_edit_and_emits_event() {
    let f = Fixture::new();
    f.edit("[ui]\nreduced_motion = true\n");
    let (service, recorded) = f.start().await;
    let handle = service.handle();
    assert!(handle.current().ui.reduced_motion, "read at start");
    assert!(
        recorded.of_type("setting.changed").is_empty(),
        "starting isn't a change"
    );
    let _watcher = watch(service.clone(), DEBOUNCE).unwrap();

    f.edit("[ui]\nreduced_motion = true\ntheme = \"light\"\n");
    let changed = recorded
        .wait_for("setting.changed", 1, Duration::from_secs(2))
        .await;
    assert_eq!(
        changed,
        [json!({"key": "ui.theme", "old": "dark", "new": "light", "source": "file"})]
    );
    assert_eq!(handle.current().ui.theme, Theme::Light);
}

// covers: CORE-03
// suite: integration
// acceptance: 0.6
#[tokio::test]
async fn hot_reload_keeps_last_valid_on_broken_file() {
    let f = Fixture::new();
    f.edit("[ui]\ntheme = \"light\"\n");
    let (service, recorded) = f.start().await;
    let handle = service.handle();
    let _watcher = watch(service.clone(), DEBOUNCE).unwrap();

    f.edit("[ui]\ntheme = \"light\"\nreduced_motion = \"yes\"\n");
    let failed = recorded
        .wait_for("settings.load_failed", 1, Duration::from_secs(2))
        .await;
    assert_eq!(failed.len(), 1);
    assert!(
        failed[0]["error"]
            .as_str()
            .unwrap()
            .contains("`ui.reduced_motion` must be true or false"),
        "{failed:?}"
    );
    assert_eq!(handle.current().ui.theme, Theme::Light, "last valid kept");
    assert!(!handle.current().ui.reduced_motion);

    // Saving the same broken text again isn't reported again.
    f.edit("[ui]\ntheme = \"light\"\nreduced_motion = \"yes\"\n");
    service.reload().await;
    tokio::time::sleep(DEBOUNCE * 3).await;
    assert_eq!(
        recorded.of_type("settings.load_failed").len(),
        1,
        "once per version"
    );

    // Fixed: it applies, and a later breakage is reported afresh.
    f.edit("[ui]\ntheme = \"light\"\nreduced_motion = true\n");
    recorded
        .wait_for("setting.changed", 1, Duration::from_secs(2))
        .await;
    assert!(handle.current().ui.reduced_motion);
    assert!(
        recorded
            .of_type("setting.changed")
            .iter()
            .all(|c| c["key"] == "ui.reduced_motion")
    );
}

// covers: CORE-03
// suite: integration
// acceptance: 0.6
#[tokio::test]
async fn theme_change_reaches_a_watch_consumer_within_a_second() {
    let f = Fixture::new();
    let (service, _recorded) = f.start().await;
    let mut ui = service.handle().watch(key("ui"));
    let mut log = service.handle().watch(key("log"));
    let _watcher = watch(service.clone(), DEBOUNCE).unwrap();

    let started = Instant::now();
    f.edit("[ui]\ntheme = \"light\"\n");
    let settings = tokio::time::timeout(Duration::from_secs(1), ui.changed())
        .await
        .expect("within a second")
        .expect("the service is running");
    assert_eq!(settings.ui.theme, Theme::Light);
    assert!(started.elapsed() < Duration::from_secs(1));
    // Consumers of other sections aren't woken.
    assert!(
        tokio::time::timeout(DEBOUNCE * 2, log.changed())
            .await
            .is_err(),
        "log didn't change"
    );
}

// covers: CORE-03
// suite: integration
#[tokio::test]
async fn set_writes_the_user_file_and_records_the_change() {
    let f = Fixture::new();
    f.edit("# mine\n[ui]\ntheme = \"dark\"   # at night\n");
    let (service, recorded) = f.start().await;

    service
        .set(&key("ui.theme"), &json!("light"))
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.dirs.user_file()).unwrap(),
        "# mine\n[ui]\ntheme = \"light\"   # at night\n"
    );
    assert_eq!(service.handle().current().ui.theme, Theme::Light);
    assert_eq!(
        recorded.of_type("setting.changed"),
        [json!({"key": "ui.theme", "old": "dark", "new": "light", "source": "user"})]
    );

    // An invalid value is refused and the file is left alone.
    let before = fs::read_to_string(f.dirs.user_file()).unwrap();
    let err = service
        .set(&key("ui.theme"), &json!("darkk"))
        .await
        .unwrap_err();
    use priya_api::error::UserFacing;
    assert!(
        err.user_message().contains("must be light or dark"),
        "{err}"
    );
    assert_eq!(fs::read_to_string(f.dirs.user_file()).unwrap(), before);

    service.unset(&key("ui.theme")).await.unwrap();
    assert_eq!(service.handle().current().ui.theme, Theme::Dark);
}

// covers: CORE-03
// suite: integration
#[tokio::test]
async fn effective_settings_add_the_project_file_and_overrides() {
    let f = Fixture::new();
    let project = f.dirs.home.as_ref().unwrap().join("code/app");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join(".priya.toml"),
        "[ai]\nmax_output_tokens = 100\n",
    )
    .unwrap();
    let (service, _recorded) = f.start().await;

    let effective = service
        .effective(
            Some(project.clone()),
            Some(json!({"ui": {"theme": "light"}})),
        )
        .await
        .unwrap();
    assert_eq!(effective.ai.max_output_tokens, 100);
    assert_eq!(effective.ui.theme, Theme::Light);
    // Only for that call.
    assert_eq!(service.handle().current().ai.max_output_tokens, 2048);
}

const CHILD_FILE: &str = "PRIYA_WRITE_LOOP_FILE";

/// Two versions of a file, big enough that a write takes a while.
fn versions() -> [String; 2] {
    [
        format!("# a\n{}", "a = \"aaaaaaaaaaaaaaaa\"\n".repeat(10_000)),
        format!("# b\n{}", "b = \"bbbbbbbbbbbbbbbb\"\n".repeat(12_000)),
    ]
}

// covers: CORE-03
// suite: integration
// acceptance: 0.6
#[test]
fn atomic_write_survives_kill_mid_write() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.toml");
    let [a, b] = versions();
    for round in 0..3_u64 {
        let mut child = LoopChild::spawn(
            "write_loop_child",
            &[(CHILD_FILE, file.as_os_str())],
            dir.path(),
        );
        child.wait_for_acks(3);
        std::thread::sleep(Duration::from_millis(round * 3));
        child.kill();
        let text = fs::read_to_string(&file).unwrap();
        assert!(text == a || text == b, "round {round}: a mix of versions");
    }
}

// covers: infra
// suite: integration
#[test]
#[ignore = "helper process for atomic_write_survives_kill_mid_write"]
fn write_loop_child() {
    let Some(file) = std::env::var_os(CHILD_FILE) else {
        return;
    };
    let stop = priya_testkit::until_stdin_closes();
    let versions = versions();
    let mut n = 0_usize;
    while !stop.load(Ordering::Relaxed) {
        write_atomic(Path::new(&file), &versions[n % 2]).unwrap();
        n += 1;
        priya_testkit::ack(n);
    }
}
