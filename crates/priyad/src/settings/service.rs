//! [`SettingsService`]: the live settings, kept in step with the files.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use priya_api::error::{ErrorKind, UserFacing};
use priya_api::events::{Actor, NewEvent, SettingChanged, SettingSource, SettingsLoadFailed};
use priya_core::settings::{
    Change, ChangeError, Content, KeyPath, Layer, Settings, SettingsErrors, Source, Target, change,
    diff, load,
};
use priya_store::EventStore;
use serde_json::Value;
use tokio::sync::{Mutex, watch};
use tracing::{info, warn};

use super::files::{self, SettingsDirs, write_atomic};

/// Where the service records what happens: the event store in priyad, a
/// list in tests.
pub trait EventSink: Send + Sync + 'static {
    /// Records `events` in order.
    fn record(&self, events: Vec<NewEvent>) -> impl Future<Output = Result<(), String>> + Send;
}

impl EventSink for EventStore {
    async fn record(&self, events: Vec<NewEvent>) -> Result<(), String> {
        self.append_all(events)
            .await
            .map(drop)
            .map_err(|e| e.to_string())
    }
}

/// The settings in force, read from the layer folders and kept current.
///
/// A file that breaks never changes them: the last valid settings stay, and
/// one `settings.load_failed` is recorded per broken version of each file.
/// Each change records one `setting.changed` per changed value.
pub struct SettingsService<S> {
    dirs: SettingsDirs,
    sink: S,
    /// One load, reload or edit at a time, so events come in order.
    state: Mutex<State>,
    publish: watch::Sender<Arc<Settings>>,
}

struct State {
    current: Arc<Settings>,
    /// Broken file versions already reported: (file, hash of its text).
    reported: HashSet<(Option<PathBuf>, u64)>,
}

/// Why `set` or `unset` didn't happen.
#[derive(Debug, thiserror::Error)]
pub enum SetError {
    #[error(transparent)]
    Invalid(#[from] ChangeError),
    #[error("{} can't be read: {reason}", path.display())]
    Unreadable { path: PathBuf, reason: String },
    #[error("couldn't save {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
}

impl UserFacing for SetError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Invalid(e) => e.kind(),
            Self::Unreadable { .. } | Self::Write { .. } => ErrorKind::Unavailable,
        }
    }

    fn user_message(&self) -> String {
        match self {
            Self::Invalid(e) => e.user_message(),
            other => other.to_string(),
        }
    }
}

impl<S: EventSink> SettingsService<S> {
    /// Loads the settings. If the files are broken, the built-in defaults
    /// apply until they are fixed.
    pub async fn start(dirs: SettingsDirs, sink: S) -> Arc<Self> {
        let defaults = Arc::new(Settings::default());
        let service = Arc::new(Self {
            dirs,
            sink,
            state: Mutex::new(State {
                current: defaults.clone(),
                reported: HashSet::new(),
            }),
            publish: watch::channel(defaults).0,
        });
        // Starting isn't a change: no `setting.changed` for what the files
        // already said.
        service.refresh(None).await;
        service
    }

    /// A handle for reading the settings and watching them change.
    pub fn handle(&self) -> SettingsHandle {
        SettingsHandle {
            rx: self.publish.subscribe(),
        }
    }

    pub fn dirs(&self) -> &SettingsDirs {
        &self.dirs
    }

    /// Reads the files again; true if they were valid.
    pub async fn reload(&self) -> bool {
        self.refresh(Some(SettingSource::File)).await
    }

    /// Sets `key` in the user's settings file, if the result is valid.
    pub async fn set(&self, key: &KeyPath, value: &Value) -> Result<(), SetError> {
        self.edit(Change::Set(key, value)).await
    }

    /// Removes `key` from the user's settings file, if the result is valid.
    pub async fn unset(&self, key: &KeyPath) -> Result<(), SetError> {
        self.edit(Change::Unset(key)).await
    }

    /// The settings for one piece of work: the files, the project file for
    /// `cwd`, and that request's own overrides. Read fresh each time, so the
    /// project file needs no watching.
    pub async fn effective(
        &self,
        cwd: Option<PathBuf>,
        overrides: Option<Value>,
    ) -> Result<Settings, SettingsErrors> {
        let read = self.read(cwd).await;
        let sources = read
            .sources
            .into_iter()
            .chain(overrides.map(Source::overrides));
        load(sources, self.dirs.home.as_deref()).map(|loaded| loaded.settings)
    }

    async fn edit(&self, change_to: Change<'_>) -> Result<(), SetError> {
        let mut state = self.state.lock().await;
        let read = self.read(None).await;
        let target = self.dirs.user_file();
        // Only the file being changed must be readable; problems elsewhere
        // don't block fixing this one.
        if let Some((path, reason)) = read.unreadable.into_iter().find(|(p, _)| *p == target) {
            return Err(SetError::Unreadable { path, reason });
        }
        let text = change(
            &read.sources,
            Target {
                layer: Layer::User,
                file: &target,
            },
            change_to,
            self.dirs.home.as_deref(),
        )?;
        let path = target.clone();
        tokio::task::spawn_blocking(move || write_atomic(&path, &text))
            .await
            .expect("writing a settings file doesn't panic")
            .map_err(|source| SetError::Write {
                path: target,
                source,
            })?;
        // What the change means with every layer, recorded as the user's.
        self.refresh_locked(&mut state, Some(SettingSource::User))
            .await;
        Ok(())
    }

    /// Reloads; `source` is what to record changes as, or none for the
    /// first load.
    async fn refresh(&self, source: Option<SettingSource>) -> bool {
        let mut state = self.state.lock().await;
        self.refresh_locked(&mut state, source).await
    }

    async fn refresh_locked(&self, state: &mut State, source: Option<SettingSource>) -> bool {
        let read = self.read(None).await;
        let failures: BTreeMap<Option<PathBuf>, String> = if read.unreadable.is_empty() {
            match load(read.sources.clone(), self.dirs.home.as_deref()) {
                Ok(loaded) => {
                    state.reported.clear();
                    self.apply(state, loaded.settings, source).await;
                    return true;
                }
                Err(errors) => by_file(&errors),
            }
        } else {
            read.unreadable
                .iter()
                .map(|(path, reason)| (Some(path.clone()), reason.clone()))
                .collect()
        };

        let mut events = Vec::new();
        for (file, error) in failures {
            // A file's version is its text; without one, the error itself.
            let text =
                read.sources
                    .iter()
                    .find(|s| s.file == file)
                    .and_then(|s| match &s.content {
                        Content::Toml(text) => Some(text.as_str()),
                        Content::Overrides(_) => None,
                    });
            let version = hash(text.unwrap_or(&error));
            if !state.reported.insert((file.clone(), version)) {
                continue; // this broken version was reported already
            }
            let shown = file
                .as_deref()
                .map(|f| f.display().to_string())
                .unwrap_or_default();
            warn!(file = %shown, %error, "settings not loaded; keeping the last valid ones");
            events.extend(event(
                Actor::System,
                &SettingsLoadFailed { file: shown, error },
            ));
        }
        self.record(events).await;
        false
    }

    /// Makes `new` current and records what changed.
    async fn apply(&self, state: &mut State, new: Settings, source: Option<SettingSource>) {
        let changes = diff(&state.current, &new);
        if changes.is_empty() {
            return;
        }
        state.current = Arc::new(new);
        self.publish.send_replace(state.current.clone());
        let Some(source) = source else { return };
        info!(changes = changes.len(), ?source, "settings changed");
        let events = changes
            .into_iter()
            .filter_map(|c| {
                event(
                    Actor::User,
                    &SettingChanged {
                        key: c.key.to_string(),
                        old: c.old,
                        new: c.new,
                        source,
                    },
                )
            })
            .collect();
        self.record(events).await;
    }

    async fn read(&self, cwd: Option<PathBuf>) -> files::Read {
        let dirs = self.dirs.clone();
        tokio::task::spawn_blocking(move || files::read(&dirs, cwd.as_deref()))
            .await
            .expect("reading settings files doesn't panic")
    }

    async fn record(&self, events: Vec<NewEvent>) {
        if events.is_empty() {
            return;
        }
        if let Err(e) = self.sink.record(events).await {
            // The settings still changed; only the record of it is missing.
            warn!(error = %e, "couldn't record settings events");
        }
    }
}

fn event<P: priya_api::events::EventPayload>(actor: Actor, payload: &P) -> Option<NewEvent> {
    NewEvent::new("settings", actor, payload)
        .inspect_err(|e| warn!(error = %e, "couldn't build a settings event"))
        .ok()
}

/// The messages for each file, one per line.
fn by_file(errors: &SettingsErrors) -> BTreeMap<Option<PathBuf>, String> {
    let mut grouped: BTreeMap<Option<PathBuf>, Vec<String>> = BTreeMap::new();
    for error in &errors.0 {
        let file = error.origin.as_ref().and_then(|o| o.file.clone());
        grouped.entry(file).or_default().push(error.user_message());
    }
    grouped
        .into_iter()
        .map(|(file, messages)| (file, messages.join("\n")))
        .collect()
}

fn hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Reads the settings in force and watches them change. Cheap to clone.
#[derive(Clone)]
pub struct SettingsHandle {
    rx: watch::Receiver<Arc<Settings>>,
}

impl SettingsHandle {
    pub fn current(&self) -> Arc<Settings> {
        self.rx.borrow().clone()
    }

    /// Waits for changes under `prefix` only (`ui`, `ai.providers`…).
    pub fn watch(&self, prefix: KeyPath) -> PrefixWatch {
        let mut rx = self.rx.clone();
        let seen = subtree(&rx.borrow_and_update(), &prefix);
        PrefixWatch { rx, prefix, seen }
    }
}

/// Changes under one prefix; see [`SettingsHandle::watch`].
pub struct PrefixWatch {
    rx: watch::Receiver<Arc<Settings>>,
    prefix: KeyPath,
    seen: Option<Value>,
}

impl PrefixWatch {
    /// The settings once something under the prefix changes; none once the
    /// service is gone.
    pub async fn changed(&mut self) -> Option<Arc<Settings>> {
        loop {
            self.rx.changed().await.ok()?;
            let settings = self.rx.borrow_and_update().clone();
            let now = subtree(&settings, &self.prefix);
            if now != self.seen {
                self.seen = now;
                return Some(settings);
            }
        }
    }
}

fn subtree(settings: &Settings, prefix: &KeyPath) -> Option<Value> {
    let all = serde_json::to_value(settings).ok()?;
    prefix
        .parts()
        .iter()
        .try_fold(&all, |node, part| node.get(part))
        .cloned()
}
