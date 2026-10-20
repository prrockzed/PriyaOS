//! [`SettingsService`]: the live settings, kept in step with the files.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use priya_api::error::{ErrorKind, UserFacing};
use priya_api::events::{Actor, NewEvent, SettingChanged, SettingSource, SettingsLoadFailed};
use priya_core::settings::{
    Change, ChangeError, Content, KeyPath, Layer, Layered, Loaded, Settings, SettingsErrors,
    Source, Target, change, diff, load,
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
    /// The settings in force and where each value came from.
    loaded: Loaded,
    /// The files they were loaded from, for `effective`.
    sources: Vec<Source>,
    /// Broken file versions already reported: (file, hash of its text).
    reported: HashSet<(Option<PathBuf>, u64)>,
}

/// Why `set` or `unset` didn't take effect.
#[derive(Debug, thiserror::Error)]
pub enum SetError {
    #[error(transparent)]
    Invalid(#[from] ChangeError),
    #[error("{} can't be read: {reason}", path.display())]
    Unreadable { path: PathBuf, reason: String },
    #[error("couldn't save {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    /// Saved, but another file's problems keep the settings from loading.
    #[error("saved, but it isn't in force until these are fixed:\n{problems}")]
    NotInForce { problems: String },
}

impl UserFacing for SetError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Invalid(e) => e.kind(),
            Self::NotInForce { .. } => ErrorKind::InvalidInput,
            Self::Unreadable { .. } | Self::Write { .. } => ErrorKind::Unavailable,
        }
    }

    fn user_message(&self) -> String {
        match self {
            Self::Invalid(e) => e.user_message(),
            Self::NotInForce { problems } => {
                format!("Saved, but it isn't in force until these are fixed:\n{problems}")
            }
            other => other.to_string(),
        }
    }
}

impl<S: EventSink> SettingsService<S> {
    /// Loads the settings. A broken file is left out until it is fixed; the
    /// rest still apply.
    pub async fn start(dirs: SettingsDirs, sink: S) -> Arc<Self> {
        let user = dirs.user.clone();
        tokio::task::spawn_blocking(move || files::sweep_temporary_files(&user))
            .await
            .expect("sweeping doesn't panic");
        let defaults = Settings::default();
        let service = Arc::new(Self {
            dirs,
            sink,
            state: Mutex::new(State {
                loaded: Loaded {
                    settings: defaults.clone(),
                    layered: Layered::default(),
                },
                sources: Vec::new(),
                reported: HashSet::new(),
            }),
            publish: watch::channel(Arc::new(defaults)).0,
        });
        // Starting isn't a change: no `setting.changed` for what the files
        // already said.
        let _ = service.refresh(None).await;
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
        self.refresh(Some(SettingSource::File)).await.is_ok()
    }

    /// Sets `key` in the user's settings file, if the result is valid.
    pub async fn set(&self, key: &KeyPath, value: &Value) -> Result<(), SetError> {
        self.edit(Change::Set(key, value)).await
    }

    /// Removes `key` from the user's settings file, if the result is valid.
    pub async fn unset(&self, key: &KeyPath) -> Result<(), SetError> {
        self.edit(Change::Unset(key)).await
    }

    /// The settings for one piece of work: the files in force, the project
    /// file for `cwd`, and that request's own overrides. The project file is
    /// read each time, so it needs no watching; a broken one is reported.
    pub async fn effective(
        &self,
        cwd: Option<PathBuf>,
        overrides: Option<Value>,
    ) -> Result<Settings, SettingsErrors> {
        let mut sources = self.state.lock().await.sources.clone();
        if let Some(cwd) = cwd {
            let dirs = self.dirs.clone();
            let project = tokio::task::spawn_blocking(move || files::read_project(&dirs, &cwd))
                .await
                .expect("reading a settings file doesn't panic");
            for (path, reason) in project.unreadable {
                warn!(file = %path.display(), %reason, "project settings can't be read; left out");
            }
            sources.extend(project.sources);
        }
        sources.extend(overrides.map(Source::overrides));
        load(sources, self.dirs.home.as_deref()).map(|loaded| loaded.settings)
    }

    async fn edit(&self, change_to: Change<'_>) -> Result<(), SetError> {
        let mut state = self.state.lock().await;
        let read = self.read().await;
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
        let written = tokio::task::spawn_blocking(move || write_atomic(&path, &text))
            .await
            .expect("writing a settings file doesn't panic");
        // Whatever the write did, match the disk; changes count as the
        // user's own.
        let in_force = self
            .refresh_locked(&mut state, Some(SettingSource::User))
            .await;
        written.map_err(|source| SetError::Write {
            path: target,
            source,
        })?;
        in_force.map_err(|problems| SetError::NotInForce { problems })
    }

    /// Reloads; `source` is what to record changes as, or none for the
    /// first load.
    async fn refresh(&self, source: Option<SettingSource>) -> Result<(), String> {
        let mut state = self.state.lock().await;
        self.refresh_locked(&mut state, source).await
    }

    /// Reads the files and applies them if they are valid. Otherwise the
    /// problems are reported (once per file version) and returned.
    async fn refresh_locked(
        &self,
        state: &mut State,
        source: Option<SettingSource>,
    ) -> Result<(), String> {
        let read = self.read().await;
        let home = self.dirs.home.as_deref();
        let mut failures: BTreeMap<Option<PathBuf>, String> = read
            .unreadable
            .iter()
            .map(|(path, reason)| (Some(path.clone()), reason.clone()))
            .collect();
        if failures.is_empty() {
            match load(read.sources.clone(), home) {
                Ok(loaded) => {
                    state.reported.clear();
                    self.apply(state, loaded, read.sources, source).await;
                    return Ok(());
                }
                Err(errors) => failures = by_file(&errors),
            }
        }
        self.report(state, &read.sources, &failures).await;
        if source.is_none() {
            // At start nothing valid is kept yet: use every file that is
            // fine, so one broken file doesn't drop the administrator's.
            let fine: Vec<Source> = read
                .sources
                .into_iter()
                .filter(|s| !failures.contains_key(&s.file))
                .collect();
            if let Ok(loaded) = load(fine.clone(), home) {
                self.apply(state, loaded, fine, None).await;
            }
        }
        Err(failures.into_values().collect::<Vec<_>>().join("\n"))
    }

    /// Records one `settings.load_failed` per broken file version.
    async fn report(
        &self,
        state: &mut State,
        sources: &[Source],
        failures: &BTreeMap<Option<PathBuf>, String>,
    ) {
        let mut events = Vec::new();
        for (file, error) in failures {
            // A file's version is its text; without one, the error itself.
            let text = sources
                .iter()
                .find(|s| s.file == *file)
                .and_then(|s| match &s.content {
                    Content::Toml(text) => Some(text.as_str()),
                    Content::Overrides(_) => None,
                });
            let version = hash(text.unwrap_or(error));
            if !state.reported.insert((file.clone(), version)) {
                continue; // this broken version was reported already
            }
            let shown = match file {
                Some(file) => shown(file, self.dirs.home.as_deref()),
                None => "your settings".into(),
            };
            warn!(file = %shown, %error, "settings not loaded; keeping the last valid ones");
            events.extend(event(
                Actor::System,
                &SettingsLoadFailed {
                    file: shown,
                    error: error.clone(),
                },
            ));
        }
        self.record(events).await;
    }

    /// Makes `loaded` current and records what changed.
    async fn apply(
        &self,
        state: &mut State,
        loaded: Loaded,
        sources: Vec<Source>,
        source: Option<SettingSource>,
    ) {
        let changes = diff(&state.loaded.settings, &loaded.settings);
        let old = std::mem::replace(&mut state.loaded, loaded);
        state.sources = sources;
        if changes.is_empty() {
            return;
        }
        self.publish
            .send_replace(Arc::new(state.loaded.settings.clone()));
        let Some(source) = source else { return };
        info!(changes = changes.len(), ?source, "settings changed");
        let events = changes
            .into_iter()
            .filter_map(|c| {
                let actor = match source {
                    SettingSource::File => actor_of(&state.loaded.layered, &old.layered, &c.key),
                    _ => Actor::User,
                };
                event(
                    actor,
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

    async fn read(&self) -> files::Read {
        let dirs = self.dirs.clone();
        tokio::task::spawn_blocking(move || files::read(&dirs))
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

/// Who changed a value found changed on disk: the system for the shipped
/// defaults and `/etc`, the owner otherwise.
fn actor_of(new: &Layered, old: &Layered, key: &KeyPath) -> Actor {
    let layer = new
        .origin(key)
        .or_else(|| old.origin(key))
        .map(|origin| origin.layer);
    match layer {
        Some(Layer::Defaults | Layer::System) => Actor::System,
        _ => Actor::User,
    }
}

/// `path` with the home folder written as `~`.
fn shown(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
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
