//! Settings layers and how they merge (02-core §6.1).
//!
//! Layers, lowest first: shipped defaults, system (`/etc/priya`), user
//! (`~/.config/priya`), project (`.priya.toml`) and a request's own overrides.
//! Files within a layer apply in path order. Tables merge key by key;
//! arrays and single values replace what was there. Every value, and every
//! table written out in a file, remembers the layer, file and line it came
//! from.
//!
//! Each file's `settings_version` is checked first: a file from a newer
//! PriyaOS is refused, and an older one is migrated in memory before it
//! merges.
//!
//! Reading the files is the caller's job, so this stays pure: it takes their
//! text as [`Source`]s.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};
use toml_edit::{Document, Item, TableLike};

use super::error::{Problem, SettingsError};
use super::key::KeyPath;
use super::migrate::{MigrateError, Migrated, migrate};

/// Where a setting can come from, lowest priority first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Defaults,
    System,
    User,
    Project,
    /// Overrides given with one API call, for that call only.
    Task,
}

/// One input to the merge: a settings file's text, or a call's overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub layer: Layer,
    pub file: Option<PathBuf>,
    pub content: Content,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Toml(String),
    /// `SettingsOverrides` from an API call: a JSON object of settings. Its
    /// keys are single parts: `{"ui": {"theme": "light"}}`, not
    /// `{"ui.theme": …}`.
    Overrides(Value),
}

impl Source {
    pub fn toml(layer: Layer, file: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self {
            layer,
            file: Some(file.into()),
            content: Content::Toml(text.into()),
        }
    }

    pub fn overrides(settings: Value) -> Self {
        Self {
            layer: Layer::Task,
            file: None,
            content: Content::Overrides(settings),
        }
    }
}

/// Where a setting came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Origin {
    pub layer: Layer,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
}

/// The merged settings tree, before it is checked, and where its values
/// came from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layered {
    tree: Map<String, Value>,
    origins: BTreeMap<KeyPath, Origin>,
}

impl Layered {
    /// Merges `sources` in layer order, then path order. Every source that
    /// can't be read is reported, not just the first.
    pub(crate) fn merge(
        sources: impl IntoIterator<Item = Source>,
        home: Option<&Path>,
    ) -> Result<Self, Vec<SettingsError>> {
        let mut sources: Vec<Source> = sources.into_iter().collect();
        sources.sort_by(|a, b| (a.layer, &a.file).cmp(&(b.layer, &b.file)));

        let mut merged = Self::default();
        let mut errors = Vec::new();
        for source in &sources {
            let mut merger = Merger {
                layered: &mut merged,
                source,
                home,
            };
            if let Err(error) = merger.merge() {
                errors.push(*error);
            }
        }
        if errors.is_empty() {
            Ok(merged)
        } else {
            Err(errors)
        }
    }

    /// The merged tree.
    pub fn tree(&self) -> &Map<String, Value> {
        &self.tree
    }

    /// Where each value, and each table written out in a file, came from.
    pub fn origins(&self) -> &BTreeMap<KeyPath, Origin> {
        &self.origins
    }

    /// Where `key` came from: its own origin, else that of the first value
    /// below it, else that of the nearest table holding it.
    pub fn origin(&self, key: &KeyPath) -> Option<&Origin> {
        if let Some(origin) = self.origins.get(key) {
            return Some(origin);
        }
        if let Some((_, origin)) = self
            .origins
            .range(key.clone()..)
            .next()
            .filter(|(k, _)| k.starts_with(key))
        {
            return Some(origin);
        }
        let mut parent = key.parent();
        while let Some(up) = parent {
            if let Some(origin) = self.origins.get(&up) {
                return Some(origin);
            }
            parent = up.parent();
        }
        None
    }

    /// Puts `value` at `key`, replacing whatever was there along with the
    /// origins of anything below it.
    fn set(&mut self, key: &KeyPath, value: Value, origin: Origin) {
        let (last, parents) = key.parts().split_last().expect("keys aren't empty");
        let table = table_at(&mut self.tree, &mut self.origins, parents);
        table.insert(last.clone(), value);
        forget(&mut self.origins, key);
        self.origins.insert(key.clone(), origin);
    }

    /// Makes sure a table is at `key`, noting where it was written if known.
    fn table(&mut self, key: &KeyPath, origin: Option<Origin>) {
        table_at(&mut self.tree, &mut self.origins, key.parts());
        if let Some(origin) = origin {
            self.origins.insert(key.clone(), origin);
        }
    }
}

/// The table at `parts`, made if needed. A value in the way becomes an
/// empty table and loses its origin.
fn table_at<'t>(
    tree: &'t mut Map<String, Value>,
    origins: &mut BTreeMap<KeyPath, Origin>,
    parts: &[String],
) -> &'t mut Map<String, Value> {
    let mut table = tree;
    for (i, part) in parts.iter().enumerate() {
        let slot = table
            .entry(part.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        if !slot.is_object() {
            *slot = Value::Object(Map::new());
            origins.remove(&KeyPath::from_parts(parts[..=i].iter().cloned()));
        }
        table = slot.as_object_mut().expect("made a table above");
    }
    table
}

/// Drops the origins of `key` and everything below it. Keys sort with their
/// descendants right after them, so this touches only what it removes.
fn forget(origins: &mut BTreeMap<KeyPath, Origin>, key: &KeyPath) {
    let doomed: Vec<KeyPath> = origins
        .range(key.clone()..)
        .take_while(|(k, _)| k.starts_with(key))
        .map(|(k, _)| k.clone())
        .collect();
    for k in doomed {
        origins.remove(&k);
    }
}

/// Merges one source into the tree.
struct Merger<'a> {
    layered: &'a mut Layered,
    source: &'a Source,
    home: Option<&'a Path>,
}

impl Merger<'_> {
    fn merge(&mut self) -> Result<(), Box<SettingsError>> {
        match &self.source.content {
            Content::Toml(text) => self.toml(text),
            Content::Overrides(Value::Object(settings)) => {
                self.json(settings, &KeyPath::default());
                Ok(())
            }
            Content::Overrides(_) => Err(self.error(
                None,
                None,
                Problem::Other(
                    "must be settings by name, like {\"ui\": {\"theme\": \"light\"}}".into(),
                ),
            )),
        }
    }

    fn origin(&self, line: Option<u32>) -> Origin {
        Origin {
            layer: self.source.layer,
            file: self.source.file.clone(),
            line,
        }
    }

    fn error(
        &self,
        line: Option<u32>,
        key: Option<KeyPath>,
        problem: Problem,
    ) -> Box<SettingsError> {
        Box::new(SettingsError::new(
            Some(self.origin(line)),
            key,
            problem,
            self.home,
        ))
    }

    fn toml(&mut self, text: &str) -> Result<(), Box<SettingsError>> {
        let lines = Lines::new(text);
        let document = Document::parse(text.to_owned()).map_err(|e| {
            let span = e.span();
            let line = span.clone().map(|span| lines.line(span.start));
            let near = span.and_then(|span| text.get(span));
            self.error(line, None, syntax(e.message(), near))
        })?;
        let version_line = document
            .as_table()
            .get_key_value("settings_version")
            .and_then(|(k, _)| k.span())
            .map(|span| lines.line(span.start));
        match migrate(text) {
            Ok(Migrated::Current) => self.document(&document, &Places::Offsets(&lines)),
            Ok(Migrated::Upgraded { text, .. }) => {
                // Lines come from the file as the owner wrote it: a key that
                // survived the migration keeps its line; a new one has none.
                let original = lines_by_key(&document, &lines);
                let document = Document::parse(text)
                    .map_err(|e| self.error(None, None, syntax(e.message(), None)))?;
                self.document(&document, &Places::Original(&original))
            }
            // The whole file is too new, not one line of it.
            Err(MigrateError::Newer { found, supported }) => {
                Err(self.error(None, None, Problem::TooNew { found, supported }))
            }
            Err(MigrateError::BadVersion) => Err(self.error(
                version_line,
                Some(KeyPath::from_parts(["settings_version"])),
                Problem::Invalid {
                    expected: "a whole number of 1 or more".into(),
                    found: None,
                },
            )),
            Err(other) => Err(self.error(None, None, Problem::Other(other.to_string()))),
        }
    }

    fn document(
        &mut self,
        document: &Document<String>,
        places: &Places<'_>,
    ) -> Result<(), Box<SettingsError>> {
        self.table(document.as_table(), &KeyPath::default(), places)
            .map_err(|(key, line, message)| self.error(line, Some(key), Problem::Other(message)))
    }

    /// A TOML table: tables merge, everything else replaces.
    fn table(
        &mut self,
        table: &dyn TableLike,
        prefix: &KeyPath,
        places: &Places<'_>,
    ) -> Result<(), (KeyPath, Option<u32>, String)> {
        for (key, item) in table.iter() {
            let path = prefix.child(key);
            let line = places.line(&path, offset(table, key, item));
            match item.as_table_like() {
                Some(inner) if !item.is_array_of_tables() => {
                    self.layered
                        .table(&path, line.map(|l| self.origin(Some(l))));
                    self.table(inner, &path, places)?;
                }
                _ => {
                    let value = to_json(item).map_err(|message| (path.clone(), line, message))?;
                    let origin = self.origin(line);
                    self.layered.set(&path, value, origin);
                }
            }
        }
        Ok(())
    }

    /// JSON overrides: objects merge, everything else replaces.
    fn json(&mut self, table: &Map<String, Value>, prefix: &KeyPath) {
        for (key, value) in table {
            let path = prefix.child(key);
            match value {
                Value::Object(inner) => {
                    self.layered.table(&path, Some(self.origin(None)));
                    self.json(inner, &path);
                }
                _ => {
                    let origin = self.origin(None);
                    self.layered.set(&path, value.clone(), origin);
                }
            }
        }
    }
}

/// Where an item was written: a value's key, or a table's `[header]` (or
/// key, for dotted and inline tables). An implicit table (`ui` in
/// `[ui.bar]`) wasn't written anywhere.
fn offset(table: &dyn TableLike, key: &str, item: &Item) -> Option<usize> {
    let key_offset = table
        .get_key_value(key)
        .and_then(|(k, _)| k.span())
        .map(|span| span.start);
    match item.as_table_like() {
        Some(_) if !item.is_array_of_tables() => {
            if item.as_table().is_some_and(|t| t.is_implicit()) {
                None
            } else {
                item.span().map(|span| span.start).or(key_offset)
            }
        }
        _ => key_offset,
    }
}

/// How to find an item's line.
enum Places<'a> {
    /// From its offset in the text being merged.
    Offsets(&'a Lines),
    /// From the file as written, by key, when the merged text was migrated.
    Original(&'a BTreeMap<KeyPath, u32>),
}

impl Places<'_> {
    fn line(&self, key: &KeyPath, offset: Option<usize>) -> Option<u32> {
        match self {
            Self::Offsets(lines) => offset.map(|at| lines.line(at)),
            Self::Original(by_key) => by_key.get(key).copied(),
        }
    }
}

/// The line of every written key and table in `document`.
fn lines_by_key(document: &Document<String>, lines: &Lines) -> BTreeMap<KeyPath, u32> {
    fn walk(
        table: &dyn TableLike,
        prefix: &KeyPath,
        lines: &Lines,
        out: &mut BTreeMap<KeyPath, u32>,
    ) {
        for (key, item) in table.iter() {
            let path = prefix.child(key);
            if let Some(at) = offset(table, key, item) {
                out.insert(path.clone(), lines.line(at));
            }
            if let Some(inner) = item.as_table_like().filter(|_| !item.is_array_of_tables()) {
                walk(inner, &path, lines, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(document.as_table(), &KeyPath::default(), lines, &mut out);
    out
}

/// The parser's message in plain words, where a common mistake is known.
/// `near` is the text the parser pointed at, if any.
fn syntax(details: &str, near: Option<&str>) -> Problem {
    let lower = details.to_lowercase();
    let message = if lower.contains("must be quoted") {
        "text needs quotes, like theme = \"dark\"".to_owned()
    } else if lower.contains("duplicate key") {
        match near.map(str::trim).filter(|key| !key.is_empty()) {
            Some(key) => format!("`{key}` is set twice"),
            None => "a key is set twice".to_owned(),
        }
    } else if lower.contains("invalid table header") || lower.contains("expected `]`") {
        "a section name needs closing brackets, like [ui]".to_owned()
    } else {
        details.to_owned()
    };
    Problem::Syntax {
        message,
        details: details.to_owned(),
    }
}

/// A TOML value as JSON. Dates and times become RFC 3339 text, so nothing
/// downstream sees TOML-only types.
fn to_json(item: &Item) -> Result<Value, String> {
    match item {
        Item::None => Ok(Value::Null),
        Item::Value(value) => value_to_json(value),
        Item::Table(table) => table_to_json(table),
        Item::ArrayOfTables(tables) => tables.iter().map(|t| table_to_json(t)).collect(),
    }
}

fn table_to_json(table: &dyn TableLike) -> Result<Value, String> {
    table
        .iter()
        .map(|(key, item)| Ok((key.to_owned(), to_json(item)?)))
        .collect::<Result<Map<_, _>, String>>()
        .map(Value::Object)
}

fn value_to_json(value: &toml_edit::Value) -> Result<Value, String> {
    use toml_edit::Value as V;
    Ok(match value {
        V::String(s) => Value::String(s.value().clone()),
        V::Integer(i) => Value::from(*i.value()),
        V::Float(f) => serde_json::Number::from_f64(*f.value())
            .map(Value::Number)
            .ok_or_else(|| format!("{} isn't a number PriyaOS can use", f.value()))?,
        V::Boolean(b) => Value::Bool(*b.value()),
        V::Datetime(d) => Value::String(d.value().to_string()),
        V::Array(items) => items.iter().map(value_to_json).collect::<Result<_, _>>()?,
        V::InlineTable(table) => table_to_json(table)?,
    })
}

/// Turns byte offsets into 1-based line numbers.
pub(crate) struct Lines {
    /// The offset just after each newline.
    starts: Vec<usize>,
}

impl Lines {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            starts: text.match_indices('\n').map(|(i, _)| i + 1).collect(),
        }
    }

    pub(crate) fn line(&self, offset: usize) -> u32 {
        let before = self.starts.partition_point(|&start| start <= offset);
        u32::try_from(before + 1).unwrap_or(u32::MAX)
    }
}

/// The project settings file for work in `cwd`: the nearest `.priya.toml`
/// in `cwd` or a folder above it, but never above `home`. Outside the home
/// folder only `cwd` itself is looked at. Both paths must be absolute and
/// free of `..`, since the check is by name; anything else finds nothing.
pub fn project_file(cwd: &Path, home: &Path, is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    const NAME: &str = ".priya.toml";
    let plain = |path: &Path| {
        path.is_absolute()
            && path
                .components()
                .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
    };
    if !plain(cwd) || !plain(home) {
        return None;
    }
    if !cwd.starts_with(home) {
        let candidate = cwd.join(NAME);
        return is_file(&candidate).then_some(candidate);
    }
    cwd.ancestors()
        .take_while(|dir| dir.starts_with(home))
        .map(|dir| dir.join(NAME))
        .find(|candidate| is_file(candidate))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::json;

    use super::*;
    use crate::settings::{Theme, load};

    const HOME: &str = "/home/p";

    fn file(layer: Layer, path: &str, text: &str) -> Source {
        Source::toml(layer, path, text)
    }

    fn merged(sources: Vec<Source>) -> Layered {
        Layered::merge(sources, Some(Path::new(HOME))).unwrap()
    }

    fn key(text: &str) -> KeyPath {
        KeyPath::parse(text).unwrap()
    }

    fn origin(layered: &Layered, text: &str) -> (Layer, Option<String>, Option<u32>) {
        let origin = layered.origin(&key(text)).expect(text);
        (
            origin.layer,
            origin.file.as_ref().map(|f| f.display().to_string()),
            origin.line,
        )
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn layers_override_in_order() {
        let sources = vec![
            // Given out of order on purpose: layers, then file names, decide.
            file(
                Layer::Project,
                "/home/p/code/.priya.toml",
                "[ui.bar]\nwidth_px = 900\n",
            ),
            file(
                Layer::User,
                "/home/p/.config/priya/20-b.toml",
                "[ui]\ntheme = \"dark\"\n",
            ),
            file(
                Layer::User,
                "/home/p/.config/priya/10-a.toml",
                "[ui]\ntheme = \"light\"\nreduced_motion = true\n",
            ),
            file(
                Layer::System,
                "/etc/priya/settings.toml",
                "[ui.bar]\nwidth_px = 800\n[ui]\ntheme = \"light\"\n",
            ),
            file(
                Layer::Defaults,
                "/usr/share/priya/defaults/00-core.toml",
                include_str!("../../../../data/defaults/00-core.toml"),
            ),
        ];
        let loaded = load(sources, Some(Path::new(HOME))).unwrap();
        assert_eq!(loaded.settings.ui.bar.width_px, 900, "project beats system");
        assert_eq!(loaded.settings.ui.theme, Theme::Dark, "20-b beats 10-a");
        assert!(loaded.settings.ui.reduced_motion, "kept from 10-a");
        assert_eq!(loaded.settings.ai.max_output_tokens, 2048, "from defaults");
        let layered = &loaded.layered;
        assert_eq!(origin(layered, "ui.bar.width_px").0, Layer::Project);
        assert_eq!(
            origin(layered, "ui.theme").1.as_deref(),
            Some("/home/p/.config/priya/20-b.toml")
        );
        assert_eq!(origin(layered, "log.level").0, Layer::Defaults);
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn tables_merge_arrays_replace() {
        let layered = merged(vec![
            file(
                Layer::System,
                "/etc/priya/settings.toml",
                "[modules]\nenabled = [\"a\", \"b\"]\n[ai.providers.anthropic]\nkind = \"anthropic\"\n",
            ),
            file(
                Layer::User,
                "/home/p/.config/priya/settings.toml",
                "[modules]\nenabled = [\"c\"]\n[ai.providers.ollama]\nkind = \"openai_compatible\"\n",
            ),
        ]);
        let tree = Value::Object(layered.tree().clone());
        assert_eq!(tree["modules"]["enabled"], json!(["c"]), "arrays replace");
        let providers: BTreeSet<&String> = tree["ai"]["providers"]
            .as_object()
            .unwrap()
            .keys()
            .collect();
        assert_eq!(providers.len(), 2, "tables merge: {providers:?}");

        // A value and a table can replace each other; stale origins go.
        let layered = merged(vec![
            file(
                Layer::System,
                "/etc/priya/a.toml",
                "[modules.x]\ny = 1\nz = 2\n",
            ),
            file(
                Layer::User,
                "/home/p/.config/priya/a.toml",
                "[modules]\nx = 5\n",
            ),
        ]);
        assert_eq!(layered.tree()["modules"]["x"], json!(5));
        assert!(
            layered
                .origins()
                .keys()
                .all(|k| k == &key("modules.x") || !k.starts_with(&key("modules.x")))
        );
        let layered = merged(vec![
            file(Layer::System, "/etc/priya/a.toml", "[modules]\nx = 5\n"),
            file(
                Layer::User,
                "/home/p/.config/priya/a.toml",
                "[modules.x]\ny = 1\n",
            ),
        ]);
        assert_eq!(layered.tree()["modules"]["x"], json!({"y": 1}));
        assert_eq!(
            origin(&layered, "modules.x").2,
            Some(1),
            "now the table's header"
        );
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn project_layer_stops_at_home() {
        let files: BTreeSet<PathBuf> = [
            "/home/.priya.toml",
            "/home/p/.priya.toml",
            "/home/p/code/app/.priya.toml",
            "/srv/x/.priya.toml",
            "/srv/.priya.toml",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect();
        let find = |cwd: &str| {
            project_file(Path::new(cwd), Path::new(HOME), |p| files.contains(p))
                .map(|p| p.display().to_string())
        };
        assert_eq!(
            find("/home/p/code/app/src").as_deref(),
            Some("/home/p/code/app/.priya.toml"),
            "the nearest one"
        );
        assert_eq!(
            find("/home/p/notes").as_deref(),
            Some("/home/p/.priya.toml")
        );
        assert_eq!(find("/home/p").as_deref(), Some("/home/p/.priya.toml"));
        // Never above home, even though /home/.priya.toml exists.
        let elsewhere = project_file(Path::new("/home/q/x"), Path::new(HOME), |p| {
            files.contains(p)
        });
        assert_eq!(elsewhere, None);
        // Outside home only the folder itself counts.
        assert_eq!(find("/srv/x").as_deref(), Some("/srv/x/.priya.toml"));
        assert_eq!(find("/srv/x/deeper"), None);
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn task_overrides_apply_only_to_that_call() {
        let user = || {
            file(
                Layer::User,
                "/home/p/.config/priya/settings.toml",
                "[ai]\nmax_output_tokens = 1000\n",
            )
        };
        let with = load(
            vec![
                user(),
                Source::overrides(
                    json!({"ai": {"max_output_tokens": 50}, "ui": {"theme": "light"}}),
                ),
            ],
            None,
        )
        .unwrap();
        assert_eq!(with.settings.ai.max_output_tokens, 50);
        assert_eq!(with.settings.ui.theme, Theme::Light);
        let origin = with.layered.origin(&key("ai.max_output_tokens")).unwrap();
        assert_eq!((origin.layer, &origin.file), (Layer::Task, &None));

        let without = load(vec![user()], None).unwrap();
        assert_eq!(without.settings.ai.max_output_tokens, 1000);
        assert_eq!(without.settings.ui.theme, Theme::Dark);

        let err = load(vec![Source::overrides(json!(["not", "a", "table"]))], None).unwrap_err();
        assert!(err.to_string().contains("given with this request"), "{err}");
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn provenance_reports_file_and_line() {
        let text = "# my settings\n\n[ui]\ntheme = \"light\"\n\n[ui.bar]\nwidth_px = 800\n[ai]\nproviders.ollama.kind = \"openai_compatible\"\n";
        let layered = merged(vec![file(
            Layer::User,
            "/home/p/.config/priya/settings.toml",
            text,
        )]);
        assert_eq!(
            origin(&layered, "ui.theme"),
            (
                Layer::User,
                Some("/home/p/.config/priya/settings.toml".into()),
                Some(4)
            )
        );
        assert_eq!(origin(&layered, "ui.bar.width_px").2, Some(7));
        assert_eq!(origin(&layered, "ai.providers.ollama.kind").2, Some(9));
        // A table points at its header; a key below a value at the value.
        assert_eq!(origin(&layered, "ui").2, Some(3));
        assert_eq!(origin(&layered, "ui.bar").2, Some(6));
        assert_eq!(origin(&layered, "ui.theme.nested").2, Some(4));
        // An empty table still has a place.
        let layered = merged(vec![file(
            Layer::User,
            "/home/p/.config/priya/settings.toml",
            "[ui]\n\n[ai.providers.ollama]\n",
        )]);
        assert_eq!(origin(&layered, "ai.providers.ollama").2, Some(3));
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn every_broken_file_is_reported_with_its_line() {
        let err = Layered::merge(
            vec![
                file(Layer::System, "/etc/priya/a.toml", "[ui]\ntheme = \n"),
                file(
                    Layer::User,
                    "/home/p/.config/priya/b.toml",
                    "ok = 1\n[[x]\n",
                ),
                file(
                    Layer::User,
                    "/home/p/.config/priya/c.toml",
                    "[ui]\nreduced_motion = true\n",
                ),
            ],
            Some(Path::new(HOME)),
        )
        .unwrap_err();
        assert_eq!(err.len(), 2, "{err:?}");
        assert_eq!(err[0].origin.as_ref().and_then(|o| o.line), Some(2));
        assert_eq!(err[1].shown_file.as_deref(), Some("~/.config/priya/b.toml"));
        assert_eq!(err[1].origin.as_ref().and_then(|o| o.line), Some(2));
        assert!(
            err[1]
                .to_string()
                .starts_with("~/.config/priya/b.toml line 2 isn't valid TOML"),
            "{}",
            err[1]
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn toml_values_become_plain_json() {
        let layered = merged(vec![file(
            Layer::User,
            "/home/p/.config/priya/m.toml",
            "[modules.x]\nwhen = 2026-10-20T03:04:05Z\nday = 2026-10-20\nratio = 0.5\nlist = [1, \"a\"]\ninline = { a = 1 }\n[[modules.x.items]]\nn = 1\n",
        )]);
        let x = &layered.tree()["modules"]["x"];
        assert_eq!(x["when"], json!("2026-10-20T03:04:05Z"));
        assert_eq!(x["day"], json!("2026-10-20"));
        assert_eq!(x["ratio"], json!(0.5));
        assert_eq!(x["list"], json!([1, "a"]));
        assert_eq!(x["inline"], json!({"a": 1}));
        assert_eq!(x["items"], json!([{"n": 1}]));
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn quoted_keys_with_dots_stay_one_key() {
        let layered = merged(vec![
            file(
                Layer::User,
                "/home/p/.config/priya/a.toml",
                "[ai.providers.\"my.lab\"]\nkind = \"anthropic\"\n[modules.x]\n\"a.b\" = 1\n",
            ),
            Source::overrides(json!({"modules": {"y": {"c.d": 2}}})),
        ]);
        let tree = Value::Object(layered.tree().clone());
        assert_eq!(
            tree["ai"]["providers"]["my.lab"]["kind"],
            json!("anthropic")
        );
        assert_eq!(tree["modules"]["x"]["a.b"], json!(1));
        assert_eq!(tree["modules"]["y"]["c.d"], json!(2));
        assert_eq!(origin(&layered, r#"modules.x."a.b""#).2, Some(4));
        assert_eq!(
            KeyPath::from_parts(["ai", "providers", "my.lab"]).to_string(),
            r#"ai.providers."my.lab""#
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn newer_or_broken_settings_versions_are_refused_per_file() {
        let err = Layered::merge(
            vec![
                file(
                    Layer::User,
                    "/home/p/.config/priya/a.toml",
                    "# x\nsettings_version = 99\n",
                ),
                file(
                    Layer::User,
                    "/home/p/.config/priya/b.toml",
                    "settings_version = \"one\"\n",
                ),
                file(
                    Layer::User,
                    "/home/p/.config/priya/c.toml",
                    "settings_version = 1\n",
                ),
            ],
            Some(Path::new(HOME)),
        )
        .unwrap_err();
        assert_eq!(err.len(), 2, "{err:?}");
        assert_eq!(
            err[0].to_string(),
            "~/.config/priya/a.toml is for a newer version of PriyaOS (settings format 99; this one reads up to 1). Update PriyaOS, or remove the newer settings"
        );
        assert_eq!(
            err[1].to_string(),
            "`settings_version` must be a whole number of 1 or more; see ~/.config/priya/b.toml line 1"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn values_toml_has_but_json_lacks_are_reported_with_their_line() {
        let err = Layered::merge(
            vec![file(
                Layer::User,
                "/home/p/.config/priya/a.toml",
                "[modules.x]\n\nf = nan\n",
            )],
            Some(Path::new(HOME)),
        )
        .unwrap_err();
        assert_eq!(
            err[0].to_string(),
            "`modules.x.f`: NaN isn't a number PriyaOS can use, in ~/.config/priya/a.toml line 3"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn common_syntax_mistakes_are_explained_plainly() {
        let message = |text: &str| {
            Layered::merge(
                vec![file(Layer::User, "/home/p/x.toml", text)],
                Some(Path::new(HOME)),
            )
            .unwrap_err()[0]
                .to_string()
        };
        assert_eq!(
            message("[ui]\ntheme = dark\n"),
            "~/x.toml line 2 isn't valid TOML: text needs quotes, like theme = \"dark\""
        );
        let twice = message("[ui]\ntheme = \"dark\"\ntheme = \"light\"\n");
        assert_eq!(
            twice,
            "~/x.toml line 3 isn't valid TOML: `theme` is set twice"
        );
        let header = message("[ui\ntheme = \"dark\"\n");
        assert!(header.contains("line 1"), "{header}");
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn project_file_refuses_paths_that_could_climb_out_of_home() {
        let everywhere = |_: &Path| true;
        assert_eq!(
            project_file(Path::new("/home/p/../q"), Path::new(HOME), everywhere),
            None
        );
        assert_eq!(
            project_file(Path::new("code"), Path::new(HOME), everywhere),
            None
        );
        assert_eq!(
            project_file(Path::new("/home/p/x"), Path::new(HOME), everywhere),
            Some(PathBuf::from("/home/p/x/.priya.toml"))
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_migrated_file_keeps_the_lines_the_owner_wrote() {
        let original = "# c\n[ui]\ntheme = \"light\"\n";
        let lines = Lines::new(original);
        let by_key = lines_by_key(&Document::parse(original.to_owned()).unwrap(), &lines);
        // As a migration might leave it: a new first line, and a new key.
        let migrated =
            "settings_version = 2\n# c\n[ui]\ntheme = \"light\"\nreduced_motion = true\n";
        let source = file(Layer::User, "/home/p/.config/priya/a.toml", migrated);
        let mut layered = Layered::default();
        let mut merger = Merger {
            layered: &mut layered,
            source: &source,
            home: None,
        };
        merger
            .document(
                &Document::parse(migrated.to_owned()).unwrap(),
                &Places::Original(&by_key),
            )
            .unwrap();
        assert_eq!(origin(&layered, "ui.theme").2, Some(3));
        assert_eq!(origin(&layered, "ui").2, Some(2));
        assert_eq!(
            origin(&layered, "ui.reduced_motion").2,
            None,
            "not in the file"
        );
    }

    // covers: CORE-03
    // suite: perf
    #[test]
    fn merging_many_keys_stays_fast() {
        // Replacing a value must not scan every origin (it once did).
        let keys: String = (0..20_000).map(|i| format!("k{i} = {i}\n")).collect();
        let text = format!("[modules.big]\n{keys}");
        let started = std::time::Instant::now();
        let layered = merged(vec![
            file(Layer::System, "/etc/priya/a.toml", &text),
            file(Layer::User, "/home/p/.config/priya/a.toml", &text),
        ]);
        // 20,000 values, plus `modules.big`'s header.
        assert_eq!(layered.origins().len(), 20_001);
        // Generous for a debug build on a busy runner; quadratic took minutes.
        assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
    }
}
