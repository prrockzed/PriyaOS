//! Settings layers and how they merge (02-core §6.1).
//!
//! Layers, lowest first: shipped defaults, system (`/etc/priya`), user
//! (`~/.config/priya`), project (`.priya.toml`) and a request's own overrides.
//! Files within a layer apply in lexical order. Tables merge key by key;
//! arrays and single values replace what was there. Every leaf remembers the
//! layer, file and line it came from.
//!
//! Reading the files is the caller's job, so this stays pure: it takes their
//! text as [`Source`]s.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};
use toml_edit::{Document, Item, TableLike};

use super::error::{Problem, SettingsError, shown};

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
    /// `SettingsOverrides` from an API call: a JSON object of settings.
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

/// Where a leaf setting's value came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Origin {
    pub layer: Layer,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
}

/// The merged settings tree, before it is checked, and where each leaf came
/// from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layered {
    tree: Map<String, Value>,
    origins: BTreeMap<String, Origin>,
}

impl Layered {
    /// Merges `sources` in layer order, then file order. Every source that
    /// can't be read is reported, not just the first.
    pub(crate) fn merge(
        sources: impl IntoIterator<Item = Source>,
        home: Option<&Path>,
    ) -> Result<Self, Vec<SettingsError>> {
        let mut sources: Vec<Source> = sources.into_iter().collect();
        sources.sort_by(|a, b| (a.layer, &a.file).cmp(&(b.layer, &b.file)));

        let mut merged = Self::default();
        let mut errors = Vec::new();
        for source in sources {
            let error = |line, key, problem| SettingsError {
                file: source.file.clone(),
                shown_file: source.file.as_deref().map(|f| shown(f, home)),
                line,
                key,
                problem,
            };
            match &source.content {
                Content::Toml(text) => match Document::parse(text.as_str()) {
                    Ok(document) => {
                        let lines = Lines::new(text);
                        let mut merger = Merger {
                            layered: &mut merged,
                            source: &source,
                            lines: Some(&lines),
                        };
                        if let Err((key, message)) = merger.table(document.as_table(), "") {
                            errors.push(error(None, Some(key), Problem::Other(message)));
                        }
                    }
                    Err(e) => {
                        let line = e.span().map(|span| Lines::new(text).line(span.start));
                        errors.push(error(line, None, Problem::Syntax(e.message().to_owned())));
                    }
                },
                Content::Overrides(Value::Object(settings)) => {
                    let mut merger = Merger {
                        layered: &mut merged,
                        source: &source,
                        lines: None,
                    };
                    merger.json(settings, "");
                }
                Content::Overrides(_) => errors.push(error(
                    None,
                    None,
                    Problem::Other("overrides must be a table of settings".into()),
                )),
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

    /// Where each leaf came from, by dotted key.
    pub fn origins(&self) -> &BTreeMap<String, Origin> {
        &self.origins
    }

    /// Where `key` came from: its own origin if it is a leaf, otherwise that
    /// of its first leaf, otherwise that of the nearest table holding it.
    pub fn origin(&self, key: &str) -> Option<&Origin> {
        if let Some(origin) = self.origins.get(key) {
            return Some(origin);
        }
        let below = format!("{key}.");
        if let Some((_, origin)) = self
            .origins
            .range(below.clone()..)
            .next()
            .filter(|(k, _)| k.starts_with(&below))
        {
            return Some(origin);
        }
        let mut parent = key;
        while let Some((up, _)) = parent.rsplit_once('.') {
            if let Some(origin) = self.origins.get(up) {
                return Some(origin);
            }
            parent = up;
        }
        None
    }

    /// Puts `value` at `path`, making tables on the way. A value replaces
    /// whatever was there, along with the origins of anything below it.
    fn set(&mut self, path: &[&str], value: Value, origin: Origin) {
        let (last, parents) = path.split_last().expect("a key has at least one part");
        let key = path.join(".");
        self.forget(&key);
        self.origins.insert(key, origin);
        self.table_at(parents).insert((*last).to_owned(), value);
    }

    /// The table at `path`, made if needed. A value in the way becomes an
    /// empty table and loses its origin.
    fn table_at(&mut self, path: &[&str]) -> &mut Map<String, Value> {
        let mut replaced = Vec::new();
        let mut table = &mut self.tree;
        for (i, part) in path.iter().enumerate() {
            let slot = table
                .entry((*part).to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            if !slot.is_object() {
                *slot = Value::Object(Map::new());
                replaced.push(path[..=i].join("."));
            }
            table = slot.as_object_mut().expect("made a table above");
        }
        for key in replaced {
            self.origins.remove(&key);
        }
        // Walk again: the loop above borrowed the tree.
        let mut table = &mut self.tree;
        for part in path {
            table = table
                .get_mut(*part)
                .and_then(Value::as_object_mut)
                .expect("made above");
        }
        table
    }

    /// Drops the origins of `key` and everything below it.
    fn forget(&mut self, key: &str) {
        let below = format!("{key}.");
        self.origins
            .retain(|k, _| k != key && !k.starts_with(&below));
    }
}

/// Merges one source into the tree.
struct Merger<'a> {
    layered: &'a mut Layered,
    source: &'a Source,
    /// For TOML sources, to turn byte offsets into line numbers.
    lines: Option<&'a Lines>,
}

impl Merger<'_> {
    fn origin(&self, offset: Option<usize>) -> Origin {
        Origin {
            layer: self.source.layer,
            file: self.source.file.clone(),
            line: self.lines.zip(offset).map(|(lines, at)| lines.line(at)),
        }
    }

    /// A TOML table: tables merge, everything else replaces.
    fn table(&mut self, table: &dyn TableLike, prefix: &str) -> Result<(), (String, String)> {
        for (key, item) in table.iter() {
            let path = join(prefix, key);
            let offset = table
                .get_key_value(key)
                .and_then(|(k, _)| k.span())
                .map(|span| span.start);
            match item.as_table_like() {
                Some(inner) if !item.is_array_of_tables() => {
                    let parts: Vec<&str> = path.split('.').collect();
                    self.layered.table_at(&parts);
                    self.table(inner, &path)?;
                }
                _ => {
                    let value = to_json(item).map_err(|message| (path.clone(), message))?;
                    let parts: Vec<&str> = path.split('.').collect();
                    let origin = self.origin(offset);
                    self.layered.set(&parts, value, origin);
                }
            }
        }
        Ok(())
    }

    /// JSON overrides: objects merge, everything else replaces.
    fn json(&mut self, table: &Map<String, Value>, prefix: &str) {
        for (key, value) in table {
            let path = join(prefix, key);
            let parts: Vec<&str> = path.split('.').collect();
            match value {
                Value::Object(inner) => {
                    self.layered.table_at(&parts);
                    self.json(inner, &path);
                }
                _ => {
                    let origin = self.origin(None);
                    self.layered.set(&parts, value.clone(), origin);
                }
            }
        }
    }
}

fn join(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

/// A TOML value as JSON. Dates and times become RFC 3339 text.
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
            .ok_or_else(|| format!("{} isn't a usable number", f.value()))?,
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
/// folder only `cwd` itself is looked at.
pub fn project_file(cwd: &Path, home: &Path, is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    const NAME: &str = ".priya.toml";
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

    fn origin(layered: &Layered, key: &str) -> (Layer, Option<String>, Option<u32>) {
        let origin = layered.origin(key).expect(key);
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
                .all(|k| !k.starts_with("modules.x."))
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
        assert!(!layered.origins().contains_key("modules.x"));
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
        let origin = with.layered.origin("ai.max_output_tokens").unwrap();
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
        // A table points at its first leaf; a key below a leaf at the leaf.
        assert_eq!(origin(&layered, "ui.bar").2, Some(7));
        assert_eq!(origin(&layered, "ui.theme.nested").2, Some(4));
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
        assert_eq!(err[0].line, Some(2));
        assert_eq!(err[1].shown_file.as_deref(), Some("~/.config/priya/b.toml"));
        assert_eq!(err[1].line, Some(2));
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
}
