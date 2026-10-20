//! The desktop doctor checks (0.13): Quickshell is there, the Priya bar's
//! shell is running, Super+Space is free for it and its fonts are
//! installed. They ask other programs (`qs`, `systemctl --user`, `hyprctl`,
//! `fc-list`) through [`Commands`], so tests give canned answers.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use priya_api::methods::system::CheckStatus;

use crate::doctor::{BoxFuture, Check, Context, Finding};

/// The oldest Quickshell the bar is written for: `IpcHandler` with typed
/// functions and `HyprlandFocusGrab`.
pub const MIN_QUICKSHELL: (u32, u32) = (0, 2);
/// The fonts PriyaUI's tokens name, and the Arch package of each.
pub const FONTS: [(&str, &str); 3] = [
    ("Inter", "inter-font"),
    ("JetBrains Mono", "ttf-jetbrains-mono"),
    ("Noto Sans Devanagari", "noto-fonts"),
];
/// How long a check waits for a program.
pub const PATIENCE: Duration = Duration::from_secs(2);

/// What a program printed, or why it didn't.
pub type Ran = Result<Output, RunError>;

#[derive(Debug)]
pub struct Output {
    pub success: bool,
    pub stdout: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunError {
    /// The program isn't installed.
    NotFound,
    /// It couldn't start, or didn't answer in time.
    Failed(String),
}

/// Runs a program and collects what it prints: the real system, or a fake.
pub trait Commands: Send + Sync + std::fmt::Debug {
    fn run<'a>(&'a self, program: &'a str, args: &'a [&'a str]) -> BoxFuture<'a, Ran>;
}

/// The programs on this computer, each given [`PATIENCE`].
#[derive(Debug)]
pub struct System;

impl Commands for System {
    fn run<'a>(&'a self, program: &'a str, args: &'a [&'a str]) -> BoxFuture<'a, Ran> {
        Box::pin(async move {
            let run = tokio::process::Command::new(program)
                .args(args)
                .stdin(std::process::Stdio::null())
                .kill_on_drop(true)
                .output();
            match tokio::time::timeout(PATIENCE, run).await {
                Ok(Ok(out)) => Ok(Output {
                    success: out.status.success(),
                    stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                }),
                Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => Err(RunError::NotFound),
                Ok(Err(e)) => Err(RunError::Failed(format!("{program}: {e}"))),
                Err(_) => Err(RunError::Failed(format!(
                    "{program} didn't answer in {PATIENCE:?}"
                ))),
            }
        })
    }
}

fn commands(context: &Context) -> Arc<dyn Commands> {
    context.daemon.doctor.commands.clone()
}

/// "quickshell 0.2.1, revision …" or "0.2.1-1" → (0, 2).
fn version(text: &str) -> Option<(u32, u32)> {
    let word = text
        .split_whitespace()
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = word
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty());
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

pub struct QuickshellInstalled;

impl Check for QuickshellInstalled {
    fn id(&self) -> &'static str {
        "desktop.quickshell_installed"
    }

    fn title(&self) -> &'static str {
        "Quickshell, which draws the Priya bar, is installed"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let (major, minor) = MIN_QUICKSHELL;
            match commands(context).run("qs", &["--version"]).await {
                Ok(out) => match version(&out.stdout) {
                    Some(found) if found >= MIN_QUICKSHELL => Finding::new(
                        CheckStatus::Ok,
                        format!("Quickshell {}.{}.", found.0, found.1),
                    ),
                    Some(found) => Finding::new(
                        CheckStatus::Warn,
                        format!(
                            "Quickshell {}.{} is too old for the Priya bar; it needs {major}.{minor} or newer. Update it with `sudo pacman -Syu quickshell`.",
                            found.0, found.1
                        ),
                    ),
                    None => Finding::new(CheckStatus::Warn, "Quickshell's version can't be read.")
                        .details(out.stdout.trim().to_owned()),
                },
                Err(RunError::NotFound) => Finding::new(
                    CheckStatus::Warn,
                    "Quickshell isn't installed, so the Priya bar can't open. Install it with `sudo pacman -S quickshell`.",
                ),
                Err(RunError::Failed(e)) => {
                    Finding::new(CheckStatus::Warn, "Quickshell is there but can't be run.")
                        .details(e)
                }
            }
        })
    }
}

pub struct PriyaShellRunning;

impl Check for PriyaShellRunning {
    fn id(&self) -> &'static str {
        "desktop.priya_shell_running"
    }

    fn title(&self) -> &'static str {
        "The Priya bar's shell is running"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let state = commands(context)
                .run("systemctl", &["--user", "is-active", "priya-shell"])
                .await
                .map(|out| out.stdout.trim().to_owned());
            match state.as_deref() {
                Ok("active") => Finding::new(CheckStatus::Ok, "It is running."),
                Ok("failed") => Finding::new(
                    CheckStatus::Warn,
                    "The Priya bar's shell stopped after an error. See why with `journalctl --user -u priya-shell`.",
                ),
                // `inactive` also covers a unit that isn't installed yet.
                Ok(state @ ("inactive" | "activating" | "deactivating" | "reloading")) => Finding::new(
                    CheckStatus::Warn,
                    "The Priya bar's shell isn't running, so Super+Space does nothing. Start it with `systemctl --user start priya-shell`.",
                )
                .details(format!("priya-shell is {state}")),
                // No user manager to ask (a terminal outside the session).
                Ok(_) | Err(_) => {
                    Finding::new(CheckStatus::Info, "systemd's user manager can't be asked from here.")
                }
            }
        })
    }
}

pub struct ShortcutFree;

impl Check for ShortcutFree {
    fn id(&self) -> &'static str {
        "desktop.shortcut_free"
    }

    fn title(&self) -> &'static str {
        "Super+Space opens the Priya bar"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            // priyad starts before Hyprland tells systemd which instance
            // is running, so it finds the instance itself.
            let runtime = context.runtime.dir.parent().map(Path::to_path_buf);
            let instance = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
                .ok()
                .or_else(|| runtime.as_deref().and_then(hyprland_instance));
            let mut args = Vec::new();
            if let Some(instance) = &instance {
                args.extend(["--instance", instance.as_str()]);
            }
            args.extend(["binds", "-j"]);
            let binds = match commands(context).run("hyprctl", &args).await {
                Ok(out) if out.success => out.stdout,
                _ => {
                    return Finding::new(CheckStatus::Info, "Hyprland can't be reached from here.");
                }
            };
            match super_space(&binds) {
                Err(_) => Finding::new(CheckStatus::Info, "Hyprland's shortcuts can't be read."),
                Ok(actions) if actions.is_empty() => Finding::new(
                    CheckStatus::Warn,
                    "Nothing is bound to Super+Space yet. Add PriyaOS's Hyprland lines (see the install guide), then reload Hyprland.",
                ),
                Ok(actions) => match actions.iter().find(|a| !a.contains("priya-bar")) {
                    None => Finding::new(CheckStatus::Ok, "It opens the Priya bar."),
                    Some(other) => Finding::new(
                        CheckStatus::Warn,
                        "Super+Space also does something else, so it can't just open the Priya bar. Free it, or bind the Priya bar to another key (see the install guide).",
                    )
                    .details(format!("Super+Space runs: {other}")),
                },
            }
        })
    }
}

/// The newest live Hyprland instance under `runtime` (the user's
/// XDG_RUNTIME_DIR): each is a folder named by its signature, holding
/// `hyprland.lock`.
fn hyprland_instance(runtime: &Path) -> Option<String> {
    std::fs::read_dir(runtime.join("hypr"))
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let lock = entry.path().join("hyprland.lock");
            let modified = lock.metadata().and_then(|m| m.modified()).ok()?;
            Some((modified, entry.file_name().to_string_lossy().into_owned()))
        })
        .max()
        .map(|(_, signature)| signature)
}

/// What `hyprctl binds -j` binds to Super+Space outside submaps, each as
/// "dispatcher arg". Hyprland runs every one of them.
fn super_space(json: &str) -> Result<Vec<String>, String> {
    // Hyprland's modifier mask for Super alone.
    const SUPER: u64 = 64;
    let binds: Vec<serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(binds
        .iter()
        .filter(|b| {
            b["modmask"].as_u64() == Some(SUPER)
                && b["key"]
                    .as_str()
                    .is_some_and(|k| k.eq_ignore_ascii_case("space"))
                && b["submap"].as_str().unwrap_or_default().is_empty()
        })
        .map(|b| {
            format!(
                "{} {}",
                b["dispatcher"].as_str().unwrap_or_default(),
                b["arg"].as_str().unwrap_or_default()
            )
            .trim()
            .to_owned()
        })
        .collect())
}

pub struct FontsPresent;

impl Check for FontsPresent {
    fn id(&self) -> &'static str {
        "desktop.fonts_present"
    }

    fn title(&self) -> &'static str {
        "The Priya bar's fonts are installed"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let families = match commands(context).run("fc-list", &[":", "family"]).await {
                Ok(out) if out.success => out.stdout,
                _ => {
                    return Finding::new(CheckStatus::Info, "The installed fonts can't be listed.");
                }
            };
            let missing: Vec<(&str, &str)> = FONTS
                .into_iter()
                .filter(|(font, _)| {
                    !families
                        .lines()
                        .any(|line| line.split(',').any(|f| f.trim() == *font))
                })
                .collect();
            if missing.is_empty() {
                return Finding::new(
                    CheckStatus::Ok,
                    "Inter, JetBrains Mono and Noto Sans Devanagari are there.",
                );
            }
            let fonts: Vec<&str> = missing.iter().map(|(font, _)| *font).collect();
            let packages: Vec<&str> = missing.iter().map(|(_, package)| *package).collect();
            Finding::new(
                CheckStatus::Warn,
                format!(
                    "Missing: {}. Install with `sudo pacman -S {}`.",
                    fonts.join(", "),
                    packages.join(" ")
                ),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: UX-02
    // suite: unit
    #[test]
    fn versions_and_shortcuts_are_read_from_what_the_tools_print() {
        assert_eq!(
            version("quickshell 0.2.1, revision 1a2b3c, distributed by: Arch"),
            Some((0, 2))
        );
        assert_eq!(version("0.3.0-1"), Some((0, 3)));
        assert_eq!(version("0.2,"), Some((0, 2)));
        assert_eq!(version("7"), None);
        assert_eq!(version("quickshell"), None);
        let binds = r#"[
            {"modmask": 64, "key": "Return", "dispatcher": "exec", "arg": "kitty", "submap": ""},
            {"modmask": 65, "key": "space", "dispatcher": "exec", "arg": "rofi", "submap": ""},
            {"modmask": 64, "key": "SPACE", "dispatcher": "exec", "arg": "qs -p /usr/share/priya/shell ipc call priya-bar toggle", "submap": ""},
            {"modmask": 64, "key": "space", "dispatcher": "exec", "arg": "wofi", "submap": "resize"},
            {"modmask": 64, "key": "space", "dispatcher": "exec", "arg": "fuzzel"}
        ]"#;
        assert_eq!(
            super_space(binds).unwrap(),
            [
                "exec qs -p /usr/share/priya/shell ipc call priya-bar toggle",
                "exec fuzzel"
            ]
        );
        assert!(super_space("[]").unwrap().is_empty());
        assert!(super_space("not json").is_err());
    }

    // covers: UX-02
    // suite: unit
    #[test]
    fn the_newest_live_hyprland_instance_is_found() {
        let runtime = tempfile::tempdir().unwrap();
        assert_eq!(hyprland_instance(runtime.path()), None);
        for signature in ["old_1", "new_2"] {
            let dir = runtime.path().join("hypr").join(signature);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("hyprland.lock"), "").unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        // A folder without a lock isn't a running instance.
        std::fs::create_dir_all(runtime.path().join("hypr/stale_3")).unwrap();
        assert_eq!(hyprland_instance(runtime.path()).as_deref(), Some("new_2"));
    }

    // The real port: what it reports for a program that works, fails,
    // hangs or isn't there.
    // covers: UX-02
    // suite: contract
    #[tokio::test]
    async fn the_system_runs_programs_and_never_waits_long() {
        let ok = System.run("echo", &["hi"]).await.unwrap();
        assert!(ok.success);
        assert_eq!(ok.stdout, "hi\n");
        assert!(!System.run("false", &[]).await.unwrap().success);
        assert_eq!(
            System.run("priya-no-such-program", &[]).await.unwrap_err(),
            RunError::NotFound
        );
        let begun = std::time::Instant::now();
        let slow = System.run("sleep", &["30"]).await.unwrap_err();
        assert!(matches!(slow, RunError::Failed(_)));
        assert!(begun.elapsed() < PATIENCE + Duration::from_secs(2));
    }
}
