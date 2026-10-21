//! `priya doctor` (CORE-05): a registry of health checks, each of which may
//! offer a fix.
//!
//! A check only looks. A fix is either `user` scope (the owner's own files,
//! made here after the CLI asks) or `system` scope (through the root
//! helper, from 2.4; listed but never made in Phase 0). During development
//! fixes only ever touch the daemon's own folders, which tests point at a
//! `TempPriyaHome`.

use std::fs;
use std::future::Future;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use priya_api::methods::system::{
    CheckResult, CheckStatus, DoctorReport, FixInfo, FixMode, FixScope,
};
use priya_core::mounts::{Layout, parse_fstab, parse_mountinfo};

use crate::api::Daemon;
use crate::daemon::RuntimeFiles;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Free space below which `disk.free_space` warns.
pub const MIN_FREE_BYTES: u64 = 1024 * 1024 * 1024;

/// What the checks look at.
pub struct Context {
    pub daemon: Arc<Daemon>,
    pub runtime: RuntimeFiles,
    pub data_dir: PathBuf,
    /// The CLI's version, when the CLI asked.
    pub cli_version: Option<String>,
    /// `/proc/self/mountinfo` and `/etc/fstab`, or test copies.
    pub mountinfo: PathBuf,
    pub fstab: PathBuf,
}

/// What a check found.
pub struct Finding {
    pub status: CheckStatus,
    pub summary: String,
    pub details: Option<String>,
    pub fix: Option<Box<dyn Fix>>,
}

impl Finding {
    pub fn new(status: CheckStatus, summary: impl Into<String>) -> Self {
        Self {
            status,
            summary: summary.into(),
            details: None,
            fix: None,
        }
    }

    pub fn details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    fn fix(mut self, fix: impl Fix + 'static) -> Self {
        self.fix = Some(Box::new(fix));
        self
    }
}

/// One health check.
pub trait Check: Send + Sync {
    /// Stable id, such as `store.integrity`.
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding>;
}

/// A fix a check offers.
pub trait Fix: Send + Sync {
    fn scope(&self) -> FixScope;
    fn description(&self) -> String;
    fn apply<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Result<(), String>>;
}

/// The Phase 0 checks, in the order they run and are shown.
pub fn checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(VersionMatchesCli),
        Box::new(SocketPermissions),
        Box::new(StoreIntegrity),
        Box::new(SettingsValid),
        Box::new(DiskFreeSpace),
        Box::new(RootFsType),
        Box::new(BtrfsLayout),
        Box::new(crate::ai::checks::Configured),
        Box::new(crate::ai::checks::KeyPresent),
        Box::new(crate::ai::checks::ProviderReachable),
        Box::new(crate::ai::checks::SecretServiceAvailable),
        Box::new(crate::install::InstallPaths),
        Box::new(crate::install::SystemdUserUnits),
        Box::new(crate::desktop::QuickshellInstalled),
        Box::new(crate::desktop::PriyaShellRunning),
        Box::new(crate::desktop::ShortcutFree),
        Box::new(crate::desktop::HyprlandSnippetSourced),
        Box::new(crate::desktop::FontsPresent),
    ]
}

/// Runs `checks`; with `FixMode::Apply`, makes the user-scope fixes and
/// checks again.
pub async fn run(
    context: &Context,
    checks: &[Box<dyn Check>],
    mode: Option<FixMode>,
) -> DoctorReport {
    let mut report = DoctorReport {
        checks: Vec::new(),
        fixed: Vec::new(),
    };
    for check in checks {
        let mut finding = check.run(context).await;
        if mode == Some(FixMode::Apply)
            && let Some(fix) = finding.fix.as_ref().filter(|f| f.scope() == FixScope::User)
        {
            match fix.apply(context).await {
                Ok(()) => {
                    report.fixed.push(check.id().to_owned());
                    finding = check.run(context).await;
                }
                Err(e) => {
                    finding.details = Some(match finding.details {
                        Some(details) => format!("{details}\nThe fix didn't work: {e}"),
                        None => format!("The fix didn't work: {e}"),
                    });
                }
            }
        }
        report.checks.push(CheckResult {
            id: check.id().to_owned(),
            title: check.title().to_owned(),
            status: finding.status,
            summary: finding.summary,
            details: finding.details,
            fix: finding.fix.map(|fix| FixInfo {
                scope: fix.scope(),
                description: fix.description(),
            }),
        });
    }
    report
}

struct VersionMatchesCli;

impl Check for VersionMatchesCli {
    fn id(&self) -> &'static str {
        "daemon.version_matches_cli"
    }

    fn title(&self) -> &'static str {
        "The command and the background service match"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let daemon = env!("CARGO_PKG_VERSION");
            match context.cli_version.as_deref() {
                None => Finding::new(
                    CheckStatus::Info,
                    format!("The background service is version {daemon}."),
                ),
                Some(cli) if cli == daemon => {
                    Finding::new(CheckStatus::Ok, format!("Both are version {daemon}."))
                }
                Some(cli) => Finding::new(
                    CheckStatus::Warn,
                    format!(
                        "The priya command is version {cli} but the background service is {daemon}. Restart it: systemctl --user restart priyad"
                    ),
                ),
            }
        })
    }
}

struct SocketPermissions;

impl Check for SocketPermissions {
    fn id(&self) -> &'static str {
        "socket.permissions"
    }

    fn title(&self) -> &'static str {
        "Only you can talk to the background service"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let problems = socket_problems(&context.runtime);
            if problems.is_empty() {
                Finding::new(CheckStatus::Ok, "Its socket is private to you.")
            } else {
                Finding::new(
                    CheckStatus::Fail,
                    "Other users might be able to talk to the background service.",
                )
                .details(problems.join("\n"))
                .fix(PrivateSocket)
            }
        })
    }
}

/// What is wrong with the runtime folder's and socket's permissions.
fn socket_problems(files: &RuntimeFiles) -> Vec<String> {
    let mut problems = Vec::new();
    let own = fs::metadata(&files.socket).map(|m| m.uid()).ok();
    for (path, wanted) in [(&files.dir, 0o700), (&files.socket, 0o600)] {
        match fs::symlink_metadata(path) {
            Ok(meta) => {
                let mode = meta.permissions().mode() & 0o777;
                if mode != wanted {
                    problems.push(format!(
                        "{} has mode {mode:o}, not {wanted:o}",
                        path.display()
                    ));
                }
                if own.is_some_and(|uid| uid != meta.uid()) {
                    problems.push(format!("{} belongs to another user", path.display()));
                }
            }
            Err(e) => problems.push(format!("{}: {e}", path.display())),
        }
    }
    problems
}

struct PrivateSocket;

impl Fix for PrivateSocket {
    fn scope(&self) -> FixScope {
        FixScope::User
    }

    fn description(&self) -> String {
        "Make the background service's folder and socket private to you again.".into()
    }

    fn apply<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let files = &context.runtime;
            for (path, mode) in [(&files.dir, 0o700), (&files.socket, 0o600)] {
                fs::set_permissions(path, fs::Permissions::from_mode(mode))
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            }
            Ok(())
        })
    }
}

struct StoreIntegrity;

impl Check for StoreIntegrity {
    fn id(&self) -> &'static str {
        "store.integrity"
    }

    fn title(&self) -> &'static str {
        "The history of what happened is intact"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let store = &context.daemon.store;
            let quick: Result<String, _> = store
                .query(|conn| Ok(conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?))
                .await;
            match quick {
                Ok(answer) if answer == "ok" => {}
                Ok(answer) => {
                    return Finding::new(CheckStatus::Fail, "The history database is damaged.")
                        .details(answer);
                }
                Err(e) => {
                    return Finding::new(CheckStatus::Fail, "The history database can't be read.")
                        .details(e.to_string());
                }
            }
            match store.verify_chain(1..=i64::MAX).await {
                Ok(report) if report.ok => Finding::new(
                    CheckStatus::Ok,
                    format!("All {} events check out.", report.checked),
                ),
                Ok(report) => Finding::new(
                    CheckStatus::Fail,
                    "Something in the history was changed, removed or reordered.",
                )
                .details(format!(
                    "first bad event: {:?}, reason: {:?}; run priya audit verify",
                    report.first_bad_seq, report.reason
                )),
                Err(e) => Finding::new(CheckStatus::Fail, "The history can't be checked.")
                    .details(e.to_string()),
            }
        })
    }
}

struct SettingsValid;

impl Check for SettingsValid {
    fn id(&self) -> &'static str {
        "settings.valid"
    }

    fn title(&self) -> &'static str {
        "Your settings files are valid"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            match context.daemon.settings.check_files().await {
                Ok(()) => Finding::new(CheckStatus::Ok, "Every settings file is valid."),
                Err(problems) => Finding::new(
                    CheckStatus::Fail,
                    "A settings file has problems, so the last valid settings are in force.",
                )
                .details(problems),
            }
        })
    }
}

struct DiskFreeSpace;

impl Check for DiskFreeSpace {
    fn id(&self) -> &'static str {
        "disk.free_space"
    }

    fn title(&self) -> &'static str {
        "There is room for PriyaOS's data"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            match free_bytes(&context.data_dir) {
                Ok(free) if free >= MIN_FREE_BYTES => {
                    Finding::new(CheckStatus::Ok, format!("{} free.", size(free)))
                }
                Ok(free) => Finding::new(
                    CheckStatus::Warn,
                    format!("Only {} free; PriyaOS needs at least 1 GB.", size(free)),
                ),
                Err(e) => Finding::new(CheckStatus::Warn, "The free space can't be measured.")
                    .details(e.to_string()),
            }
        })
    }
}

fn free_bytes(dir: &Path) -> std::io::Result<u64> {
    let stats = rustix::fs::statvfs(dir)?;
    Ok(stats.f_bavail.saturating_mul(stats.f_frsize))
}

/// "12.3 GB", in the decimal units disks are sold in.
fn size(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss, reason = "shown rounded")]
    let gb = bytes as f64 / 1e9;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", gb * 1000.0)
    }
}

/// The mounts and fstab, or why they can't be read (not Linux).
fn layout(context: &Context) -> Result<Layout, String> {
    let mountinfo = fs::read_to_string(&context.mountinfo)
        .map_err(|e| format!("{}: {e}", context.mountinfo.display()))?;
    // A missing fstab is unusual but not fatal for these checks.
    let fstab = fs::read_to_string(&context.fstab).unwrap_or_default();
    Ok(Layout::inspect(
        &parse_mountinfo(&mountinfo),
        &parse_fstab(&fstab),
    ))
}

struct RootFsType;

impl Check for RootFsType {
    fn id(&self) -> &'static str {
        "fs.root_type"
    }

    fn title(&self) -> &'static str {
        "What the system disk uses"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            match layout(context) {
                Ok(layout) => match layout.root_fs {
                    Some(fs) => {
                        Finding::new(CheckStatus::Info, format!("The system disk uses {fs}."))
                    }
                    None => Finding::new(CheckStatus::Info, "The system disk's type isn't known."),
                },
                Err(e) => Finding::new(
                    CheckStatus::Info,
                    "This isn't a Linux system, so there's nothing to check.",
                )
                .details(e),
            }
        })
    }
}

struct BtrfsLayout;

impl Check for BtrfsLayout {
    fn id(&self) -> &'static str {
        "fs.btrfs_layout"
    }

    fn title(&self) -> &'static str {
        "The disk is ready for Priya Sandbox"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let layout = match layout(context) {
                Ok(layout) => layout,
                Err(e) => {
                    return Finding::new(
                        CheckStatus::Info,
                        "This isn't a Linux system, so there's nothing to check.",
                    )
                    .details(e);
                }
            };
            if layout.is_priyaos() {
                return Finding::new(CheckStatus::Info, "The disk has the PriyaOS layout.");
            }
            let mut details = Vec::new();
            match layout.root_fs.as_deref() {
                Some("btrfs") => {}
                Some(other) => details.push(format!("The system disk uses {other}, not btrfs.")),
                None => details.push("The system disk's type isn't known.".into()),
            }
            if !layout.missing.is_empty() {
                details.push(format!(
                    "Missing subvolumes: {}.",
                    layout.missing.join(", ")
                ));
            }
            if !layout.boot_inside_root {
                details.push("/boot is its own partition, so kernels aren't in snapshots.".into());
            }
            if layout.fstab_pins_root {
                details.push(
                    "/etc/fstab names a subvolume for /, so a previous system can't be booted."
                        .into(),
                );
            }
            if layout.fstab_uses_subvolid {
                details.push("/etc/fstab uses subvolid=, which breaks after a system swap.".into());
            }
            Finding::new(
                CheckStatus::Info,
                "Priya Sandbox needs btrfs with the PriyaOS layout — arrives in Phase 2.",
            )
            .details(details.join("\n"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-05
    // suite: unit
    #[test]
    fn sizes_read_plainly() {
        assert_eq!(size(12_345_678_901), "12.3 GB");
        assert_eq!(size(250_000_000), "250 MB");
    }
}
