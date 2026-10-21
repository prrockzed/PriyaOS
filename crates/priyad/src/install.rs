//! The install doctor checks (0.14): the package's files are all there,
//! and priyad starts with the session. A development run (priyad started
//! from a checkout) has no package to check, and says so.

use priya_api::methods::system::CheckStatus;

use crate::desktop::Commands;
use crate::doctor::{BoxFuture, Check, Context, Finding};

/// What the `priya` package installs, under the install root.
pub const CORE_FILES: [&str; 5] = [
    "usr/bin/priyad",
    "usr/bin/priya",
    "usr/lib/systemd/user/priyad.service",
    "usr/share/priya/defaults/00-core.toml",
    "usr/share/priya/prompts/answer.md",
];
/// What the `priya-shell` package installs.
pub const SHELL_FILES: [&str; 2] = [
    "usr/lib/systemd/user/priya-shell.service",
    "usr/share/priya/shell/shell.qml",
];

pub struct InstallPaths;

impl Check for InstallPaths {
    fn id(&self) -> &'static str {
        "install.paths"
    }

    fn title(&self) -> &'static str {
        "PriyaOS's installed files are all there"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let doctor = &context.daemon.doctor;
            let root = &doctor.install_root;
            if !doctor.daemon_exe.starts_with(root.join("usr/bin")) {
                return Finding::new(
                    CheckStatus::Info,
                    format!(
                        "priyad runs from {}, not from the package, so there is nothing installed to check.",
                        doctor.daemon_exe.display()
                    ),
                );
            }
            let missing = |files: &[&str]| -> Vec<String> {
                files
                    .iter()
                    .filter(|f| !root.join(f).exists())
                    .map(|f| format!("/{f}"))
                    .collect()
            };
            let core = missing(&CORE_FILES);
            if !core.is_empty() {
                return Finding::new(
                    CheckStatus::Warn,
                    "Some of PriyaOS's files are missing. Install them again with `sudo pacman -S priya`.",
                )
                .details(core.join("\n"));
            }
            let shell = missing(&SHELL_FILES);
            if !shell.is_empty() {
                return Finding::new(
                    CheckStatus::Warn,
                    "The Priya bar isn't installed. Install it with `sudo pacman -S priya-shell`.",
                )
                .details(shell.join("\n"));
            }
            Finding::new(CheckStatus::Ok, "priya and priya-shell are installed.")
        })
    }
}

pub struct SystemdUserUnits;

impl Check for SystemdUserUnits {
    fn id(&self) -> &'static str {
        "systemd.user_units"
    }

    fn title(&self) -> &'static str {
        "The background service starts with your session"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let commands = context.daemon.doctor.commands.clone();
            let enabled = state(commands.as_ref(), "is-enabled").await;
            let active = state(commands.as_ref(), "is-active").await;
            let Some(enabled) = enabled else {
                return Finding::new(
                    CheckStatus::Info,
                    "systemd's user manager can't be asked from here.",
                );
            };
            match (enabled.as_str(), active.as_deref() == Some("active")) {
                ("enabled", true) => {
                    Finding::new(CheckStatus::Ok, "priyad is enabled and running.")
                }
                (_, true) => Finding::new(
                    CheckStatus::Warn,
                    "priyad won't start by itself at your next login. Turn that on with: systemctl --user enable priyad",
                ),
                // This very priyad answers, but not as the service: a
                // development run, or one started by hand.
                (_, false) => Finding::new(
                    CheckStatus::Info,
                    "priyad isn't running as its service. To use the service: systemctl --user enable --now priyad",
                ),
            }
        })
    }
}

/// `systemctl --user <query> priyad`, or `None` when it can't be asked or
/// the unit isn't known.
async fn state(commands: &dyn Commands, query: &str) -> Option<String> {
    let out = commands
        .run("systemctl", &["--user", query, "priyad"])
        .await
        .ok()?;
    let state = out.stdout.trim().to_owned();
    (!state.is_empty() && state != "not-found").then_some(state)
}
