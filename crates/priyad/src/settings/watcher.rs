//! Live reload: the layer folders are watched, and a burst of changes (an
//! editor's save is often several) becomes one reload after a short quiet
//! spell.

use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::warn;

use super::files::is_settings_file;
use super::service::{EventSink, SettingsService};

/// How long the folders must be quiet before a reload.
pub const DEBOUNCE: Duration = Duration::from_millis(250);

/// Watches while it lives; dropping it stops watching.
pub struct SettingsWatcher {
    _watcher: RecommendedWatcher,
    task: JoinHandle<()>,
}

impl Drop for SettingsWatcher {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Starts watching the default, system and user folders. The user folder is
/// made if missing; a missing default or system folder isn't watched (one
/// added later is picked up on the next restart). Must run inside a tokio
/// runtime.
pub fn watch<S: EventSink>(
    service: Arc<SettingsService<S>>,
    debounce: Duration,
) -> notify::Result<SettingsWatcher> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            Ok(event) if event.paths.iter().any(|path| is_settings_file(path)) => {
                let _ = tx.send(());
            }
            Ok(_) => {}
            Err(e) => warn!(error = %e, "settings watcher error"),
        })?;
    let dirs = service.dirs();
    std::fs::create_dir_all(&dirs.user)?;
    for dir in [&dirs.defaults, &dirs.system, &dirs.user] {
        if dir.is_dir() {
            watcher.watch(dir, RecursiveMode::NonRecursive)?;
        }
    }
    let task = tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Wait for the burst to end.
            loop {
                match tokio::time::timeout(debounce, rx.recv()).await {
                    Ok(Some(())) => {}
                    Ok(None) => return,
                    Err(_quiet) => break,
                }
            }
            service.reload().await;
        }
    });
    Ok(SettingsWatcher {
        _watcher: watcher,
        task,
    })
}
