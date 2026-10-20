//! Live reload: the layer folders are watched, and a burst of changes (an
//! editor's save is often several) becomes one reload after a short quiet
//! spell, or after at most [`MAX_DELAY`] while changes keep coming.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::event::{AccessKind, AccessMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tracing::warn;

use super::files::is_settings_file;
use super::service::{EventSink, SettingsService};

/// How long the folders must be quiet before a reload.
pub const DEBOUNCE: Duration = Duration::from_millis(250);
/// The longest a reload waits while changes keep coming.
pub const MAX_DELAY: Duration = Duration::from_secs(1);

/// Watches while it lives; dropping it stops watching.
pub struct SettingsWatcher {
    _watcher: Arc<Mutex<RecommendedWatcher>>,
    task: JoinHandle<()>,
}

impl Drop for SettingsWatcher {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Starts watching the default, system and user folders.
///
/// - The user folder is made if missing. Its parent is watched too, so a
///   user folder that is deleted and made again is watched again.
/// - A missing default or system folder isn't watched; one added later is
///   picked up on the next restart.
/// - One reload runs right away, for changes made since the service started.
///
/// Must run inside a tokio runtime.
pub fn watch<S: EventSink>(
    service: Arc<SettingsService<S>>,
    debounce: Duration,
) -> notify::Result<SettingsWatcher> {
    let dirs = service.dirs().clone();
    let user = dirs.user.clone();
    std::fs::create_dir_all(&user)?;
    // Events may name a folder by its real path (FSEvents reports
    // `/private/var/…` for `/var/…`), so both forms count.
    let both = |dir: &Path| {
        let mut forms = vec![dir.to_path_buf()];
        forms.extend(std::fs::canonicalize(dir).ok().filter(|real| real != dir));
        forms
    };
    let folders: Vec<PathBuf> = [&dirs.defaults, &dirs.system, &dirs.user]
        .into_iter()
        .flat_map(|dir| both(dir))
        .collect();
    let users = both(&user);
    // Capacity 1: a pending reload covers any number of changes.
    let (tx, mut rx) = mpsc::channel::<()>(1);
    let rewatch = Arc::new(AtomicBool::new(false));
    let mut watcher = notify::recommended_watcher({
        let tx = tx.clone();
        let rewatch = rewatch.clone();
        move |event: notify::Result<notify::Event>| {
            let wanted = match &event {
                Ok(event) => {
                    // The user folder made, or moved back into place: its
                    // old watch is gone (on Linux).
                    if event.paths.iter().any(|p| users.contains(p))
                        && matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
                    {
                        rewatch.store(true, Ordering::Relaxed);
                    }
                    changes_settings(event, &folders, &users)
                }
                // A lost event might have been an edit: reload to be sure.
                Err(e) => {
                    warn!(error = %e, "settings watcher error; reloading");
                    true
                }
            };
            if wanted {
                let _ = tx.try_send(());
            }
        }
    })?;
    for dir in [&dirs.defaults, &dirs.system, &dirs.user] {
        if dir.is_dir() {
            watcher.watch(dir, RecursiveMode::NonRecursive)?;
        }
    }
    if let Some(parent) = user.parent().filter(|p| p.is_dir()) {
        watcher.watch(parent, RecursiveMode::NonRecursive)?;
    }
    let watcher = Arc::new(Mutex::new(watcher));
    let _ = tx.try_send(());

    let task = tokio::spawn({
        let watcher = watcher.clone();
        async move {
            while rx.recv().await.is_some() {
                // Wait for a quiet spell, but not forever.
                let deadline = Instant::now() + MAX_DELAY;
                loop {
                    let wait = debounce.min(deadline.saturating_duration_since(Instant::now()));
                    if wait.is_zero() {
                        break;
                    }
                    match tokio::time::timeout(wait, rx.recv()).await {
                        Ok(Some(())) => {}
                        Ok(None) => return,
                        Err(_quiet) => break,
                    }
                }
                if rewatch.swap(false, Ordering::Relaxed) {
                    watch_again(&watcher, &user);
                }
                service.reload().await;
            }
        }
    });
    Ok(SettingsWatcher {
        _watcher: watcher,
        task,
    })
}

/// Whether `event` may have changed a settings file. Reads don't count:
/// on Linux, opening a file is an event, and the service's own reads would
/// otherwise reload it forever.
fn changes_settings(event: &notify::Event, folders: &[PathBuf], users: &[PathBuf]) -> bool {
    if event.need_rescan() {
        return true;
    }
    let changes = matches!(
        event.kind,
        EventKind::Create(_)
            | EventKind::Modify(_)
            | EventKind::Remove(_)
            | EventKind::Access(AccessKind::Close(AccessMode::Write))
            | EventKind::Any
            | EventKind::Other
    );
    // Only files directly in a layer folder: the user folder's parent
    // (`~/.config`) is watched for the folder itself, not other apps' files.
    let in_layer = |path: &Path| {
        is_settings_file(path)
            && path
                .parent()
                .is_some_and(|dir| folders.iter().any(|f| f == dir))
    };
    changes
        && event
            .paths
            .iter()
            .any(|path| in_layer(path) || users.contains(path))
}

/// Watches the user folder again after it was made anew.
fn watch_again(watcher: &Mutex<RecommendedWatcher>, user: &Path) {
    if !user.is_dir() {
        return;
    }
    let mut watcher = watcher.lock().unwrap_or_else(|e| e.into_inner());
    // Some backends (FSEvents) don't merge a repeated path.
    let _ = watcher.unwatch(user);
    if let Err(e) = watcher.watch(user, RecursiveMode::NonRecursive) {
        warn!(error = %e, folder = %user.display(), "couldn't watch the settings folder again");
    }
}

#[cfg(test)]
mod tests {
    use notify::event::{CreateKind, DataChange, ModifyKind};

    use super::*;

    fn event(kind: EventKind, path: &str) -> notify::Event {
        notify::Event::new(kind).add_path(PathBuf::from(path))
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn only_changes_to_settings_files_count() {
        let user = Path::new("/home/p/.config/priya");
        let folders = [PathBuf::from("/etc/priya"), user.to_path_buf()];
        let users = [user.to_path_buf()];
        let changes_settings =
            |event: &notify::Event, _: &Path| changes_settings(event, &folders, &users);
        let file = "/home/p/.config/priya/settings.toml";
        let modify = EventKind::Modify(ModifyKind::Data(DataChange::Content));
        assert!(changes_settings(&event(modify, file), user));
        assert!(changes_settings(
            &event(
                EventKind::Access(AccessKind::Close(AccessMode::Write)),
                file
            ),
            user
        ));
        // Reads, other files and temporary files don't.
        assert!(!changes_settings(
            &event(EventKind::Access(AccessKind::Open(AccessMode::Read)), file),
            user
        ));
        assert!(!changes_settings(
            &event(EventKind::Access(AccessKind::Close(AccessMode::Read)), file),
            user
        ));
        assert!(!changes_settings(
            &event(modify, "/home/p/.config/kitty.conf"),
            user
        ));
        assert!(!changes_settings(
            &event(modify, "/home/p/.config/priya/.settings.toml.tmp-1-0"),
            user
        ));
        // The folder coming back counts, and so does a rescan.
        assert!(changes_settings(
            &event(
                EventKind::Create(CreateKind::Folder),
                "/home/p/.config/priya"
            ),
            user
        ));
        let rescan = notify::Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert!(changes_settings(&rescan, user));
    }
}
