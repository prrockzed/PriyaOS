//! Checking the merged settings and turning them into [`Settings`].
//!
//! First the tree is walked against the JSON Schema: unknown keys (with a
//! "did you mean") and values where a section belongs are all found at
//! once. Then it is deserialised; each failure names its key, the bad value
//! is dropped (so its default applies) and it tries again, so every problem
//! is reported in one go. The schema says what each key expects ("light or
//! dark", "a whole number from 320 to 7680"), so serde's wording never
//! reaches the owner; only our own types' messages (a strategy, a provider
//! name) are passed on. Provenance adds the file and line.

use std::path::Path;

use serde_json::{Map, Value};

use super::error::{Problem, SettingsError, SettingsErrors};
use super::key::KeyPath;
use super::layers::{Layered, Source};
use super::model::Settings;

/// Problems reported at most; past this the owner is told there are more.
const MAX_REPORTED: usize = 50;
/// A safety stop for the retry loop, well above `MAX_REPORTED` since some
/// attempts only find knock-on problems.
const MAX_ATTEMPTS: usize = 500;
/// Longer values are cut when repeated back.
const SHOWN_CHARS: usize = 40;

/// Settings that passed every check, with where each value came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub settings: Settings,
    pub layered: Layered,
}

/// Merges `sources` and checks the result. `home` only shortens paths in
/// messages to `~/…`.
pub fn load(
    sources: impl IntoIterator<Item = Source>,
    home: Option<&Path>,
) -> Result<Loaded, SettingsErrors> {
    let layered = Layered::merge(sources, home).map_err(SettingsErrors::sorted)?;
    let checker = Checker {
        schema: SchemaView::new(),
        layered: &layered,
        home,
    };
    let mut errors = Vec::new();
    // Keys reported or suggested so far, to recognise knock-on problems.
    let mut reported: Vec<KeyPath> = Vec::new();
    let mut tree = Value::Object(layered.tree().clone());
    for (key, problem) in checker.structure() {
        remove(&mut tree, &key);
        if let Problem::UnknownKey {
            suggestion: Some(other),
        } = &problem
        {
            reported.push(other.clone());
        }
        reported.push(key.clone());
        errors.push(checker.error(key, problem));
    }
    let mut attempts = 0;
    let complete = loop {
        if errors.len() >= MAX_REPORTED || attempts >= MAX_ATTEMPTS {
            break false;
        }
        attempts += 1;
        match serde_path_to_error::deserialize::<_, Settings>(&tree) {
            Ok(settings) if errors.is_empty() => return Ok(Loaded { settings, layered }),
            Ok(_) => break true,
            Err(e) => {
                let (key, in_list) = key_of(e.path());
                let message = e.into_inner().to_string();
                let problem = checker.problem(&key, value_at(&tree, &key), &message, in_list);
                let removed = remove(&mut tree, &key);
                // A table missing a field that was itself reported (a bad
                // value we removed, or a misspelling we suggested it for)
                // isn't a new problem.
                let follows = matches!(&problem, Problem::Missing { field }
                    if reported.contains(&key.child(field)));
                if !follows {
                    reported.push(key.clone());
                    errors.push(checker.error(key, problem));
                }
                if !removed {
                    break true;
                }
            }
        }
    };
    let mut errors = SettingsErrors::sorted(errors);
    if !complete {
        errors.0.push(SettingsError::new(
            None,
            None,
            Problem::Other("there are more problems; fix these first".into()),
            home,
        ));
    }
    Err(errors)
}

/// Whether `key` names a setting (or a table of them) that the schema
/// knows, set or not; anything under a module's section counts (modules
/// check their own, 0.8). The empty key is all settings.
pub fn is_setting(key: &KeyPath) -> bool {
    let schema = SchemaView::new();
    let mut node = schema.root();
    for part in key.parts() {
        match schema.child(node, part) {
            Child::Known { schema: inner, .. } => node = inner,
            Child::Open => return true,
            Child::Unknown => return false,
        }
    }
    true
}

struct Checker<'a> {
    schema: SchemaView,
    layered: &'a Layered,
    home: Option<&'a Path>,
}

impl Checker<'_> {
    fn error(&self, key: KeyPath, problem: Problem) -> SettingsError {
        let origin = self.layered.origin(&key).cloned();
        let key = (!key.is_empty()).then_some(key);
        SettingsError::new(origin, key, problem, self.home)
    }

    /// Unknown keys, and values where a section belongs.
    fn structure(&self) -> Vec<(KeyPath, Problem)> {
        let mut found = Vec::new();
        self.walk(
            self.layered.tree(),
            self.schema.root(),
            &KeyPath::default(),
            &mut found,
        );
        found
    }

    fn walk(
        &self,
        tree: &Map<String, Value>,
        node: &Value,
        prefix: &KeyPath,
        found: &mut Vec<(KeyPath, Problem)>,
    ) {
        for (key, value) in tree {
            let path = prefix.child(key);
            match self.schema.child(node, key) {
                Child::Known { schema, named } if self.schema.is_table(schema) => match value {
                    Value::Object(table) => self.walk(table, schema, &path, found),
                    // A value under a free-form name (a module section) is
                    // more likely a misspelt key than a module.
                    _ if !named && self.suggest(node, &path).is_some() => found.push((
                        path.clone(),
                        Problem::UnknownKey {
                            suggestion: self.suggest(node, &path),
                        },
                    )),
                    _ => found.push((
                        path.clone(),
                        Problem::Invalid {
                            expected: format!("a section, like [{path}]"),
                            found: show(value),
                        },
                    )),
                },
                Child::Known { .. } | Child::Open => {}
                Child::Unknown => {
                    let suggestion = self.suggest(node, &path);
                    found.push((path, Problem::UnknownKey { suggestion }));
                }
            }
        }
    }

    /// A key the owner probably meant: a near spelling in the same table,
    /// else the same name in another section (`ui.level` → `log.level`).
    fn suggest(&self, node: &Value, key: &KeyPath) -> Option<KeyPath> {
        let name = key.last()?;
        if let Some(near) = closest(name, self.schema.keys(node)) {
            let parent = key.parent().unwrap_or_default();
            return Some(parent.child(near));
        }
        self.schema
            .all_keys()
            .into_iter()
            .find(|other| other.last() == Some(name) && other != key)
    }

    /// What went wrong, in the schema's terms where it can say.
    fn problem(
        &self,
        key: &KeyPath,
        found: Option<&Value>,
        message: &str,
        in_list: bool,
    ) -> Problem {
        if let Some(field) = message
            .strip_prefix("missing field `")
            .and_then(|rest| rest.split('`').next())
        {
            return Problem::Missing {
                field: field.to_owned(),
            };
        }
        let node = self.schema.at(key);
        let expected = node.and_then(|node| {
            if in_list {
                expected_list(node)
            } else {
                expected_enum(node)
                    .or_else(|| expected_range(node))
                    .or_else(|| {
                        found
                            .is_some_and(|v| !fits_type(node, v))
                            .then(|| expected_type(node, key))
                            .flatten()
                    })
            }
        });
        let found = found.and_then(show);
        match expected {
            Some(expected) => Problem::Invalid { expected, found },
            // serde's own wording means a shape we couldn't name: say what
            // the schema wants rather than repeat it.
            None if is_serde_wording(message) => Problem::Invalid {
                expected: node
                    .and_then(|node| expected_type(node, key))
                    .unwrap_or_else(|| "something else".into()),
                found,
            },
            // The value has the right shape but our own type refused it (a
            // strategy, a provider name): its message says why.
            None => Problem::Rejected {
                message: message.to_owned(),
                found,
            },
        }
    }
}

/// Messages serde writes itself, as opposed to our types' own.
fn is_serde_wording(message: &str) -> bool {
    [
        "invalid type",
        "invalid value",
        "unknown variant",
        "invalid length",
        "unknown field",
        "expected ",
    ]
    .iter()
    .any(|start| message.starts_with(start))
}

/// The schema's type, ignoring `null` (as in `["string", "null"]`).
fn schema_type(node: &Value) -> Option<&str> {
    match node.get("type")? {
        Value::String(name) => Some(name),
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .find(|n| *n != "null"),
        _ => None,
    }
}

/// The allowed values, from `enum`, or from `oneOf` members with a `const`
/// each (how schemars writes an enum whose variants have descriptions).
fn expected_enum(node: &Value) -> Option<String> {
    let names: Vec<String> = match (node.get("enum"), node.get("oneOf")) {
        (Some(Value::Array(values)), _) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        (_, Some(Value::Array(members))) => members
            .iter()
            .map(|m| m.get("const").and_then(Value::as_str).map(str::to_owned))
            .collect::<Option<_>>()?,
        _ => return None,
    };
    (!names.is_empty()).then(|| either(&names))
}

fn expected_range(node: &Value) -> Option<String> {
    if schema_type(node)? != "integer" {
        return None;
    }
    let min = node.get("minimum").and_then(Value::as_i64);
    let max = node.get("maximum").and_then(Value::as_i64);
    Some(match (min, max) {
        (Some(min), Some(max)) => format!("a whole number from {min} to {max}"),
        (Some(min), None) => format!("a whole number of {min} or more"),
        (None, Some(max)) => format!("a whole number up to {max}"),
        (None, None) => "a whole number".into(),
    })
}

fn expected_type(node: &Value, key: &KeyPath) -> Option<String> {
    Some(match schema_type(node)? {
        "boolean" => "true or false".into(),
        "string" => "text in quotes".into(),
        "array" => expected_list(node)?,
        "object" => format!("a section, like [{key}]"),
        "number" => "a number".into(),
        "integer" => expected_range(node)?,
        _ => return None,
    })
}

/// "a list of text, like ["a", "b"]".
fn expected_list(node: &Value) -> Option<String> {
    if schema_type(node)? != "array" {
        return None;
    }
    let (items, example) = match node.get("items").and_then(schema_type) {
        Some("string") => ("text", r#"["a", "b"]"#),
        Some("integer") => ("whole numbers", "[1, 2]"),
        Some("boolean") => ("true or false values", "[true, false]"),
        _ => ("values", r#"["a", "b"]"#),
    };
    Some(format!("a list of {items}, like {example}"))
}

/// Whether `value` has the JSON type the schema asks for.
fn fits_type(node: &Value, value: &Value) -> bool {
    match schema_type(node) {
        Some("boolean") => value.is_boolean(),
        Some("string") => value.is_string(),
        Some("array") => value.is_array(),
        Some("object") => value.is_object(),
        Some("number") => value.is_number(),
        Some("integer") => value.is_i64() || value.is_u64(),
        _ => true,
    }
}

/// "a", "a or b", "a, b or c".
fn either(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// A value as the owner wrote it, short and on one line; tables and lists
/// aren't repeated back.
fn show(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let mut shown: String = text
                .chars()
                .flat_map(|c| {
                    if c.is_control() {
                        c.escape_default().collect::<Vec<_>>()
                    } else {
                        vec![c]
                    }
                })
                .collect();
            if shown.chars().count() > SHOWN_CHARS {
                shown = shown.chars().take(SHOWN_CHARS).collect::<String>() + "…";
            }
            Some(format!("'{shown}'"))
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// The key a deserialisation error is about, and whether it was inside a
/// list (lists are single values in the settings tree, so the key is the
/// list's).
fn key_of(path: &serde_path_to_error::Path) -> (KeyPath, bool) {
    use serde_path_to_error::Segment;
    let mut parts = Vec::new();
    for segment in path.iter() {
        match segment {
            Segment::Map { key } => parts.push(key.clone()),
            Segment::Seq { .. } => return (KeyPath::from_parts(parts), true),
            Segment::Enum { .. } | Segment::Unknown => {}
        }
    }
    (KeyPath::from_parts(parts), false)
}

fn value_at<'a>(tree: &'a Value, key: &KeyPath) -> Option<&'a Value> {
    key.parts()
        .iter()
        .try_fold(tree, |node, part| node.get(part))
}

/// Removes `key` from the tree; false if it wasn't there.
fn remove(tree: &mut Value, key: &KeyPath) -> bool {
    let Some((last, parents)) = key.parts().split_last() else {
        return false;
    };
    parents
        .iter()
        .try_fold(tree, |node, part| node.get_mut(part))
        .and_then(Value::as_object_mut)
        .and_then(|table| table.remove(last))
        .is_some()
}

/// The allowed key nearest to `key`, if one is close enough to be a typo.
fn closest<'a>(key: &str, allowed: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (key.chars().count() / 3).max(1);
    allowed
        .filter(|candidate| *candidate != key)
        .map(|candidate| (strsim::damerau_levenshtein(key, candidate), candidate))
        .filter(|(distance, _)| *distance <= limit)
        .min()
        .map(|(_, candidate)| candidate)
}

/// What the schema allows under one key.
enum Child<'a> {
    /// `named` when it is a listed property, not a free-form name.
    Known {
        schema: &'a Value,
        named: bool,
    },
    /// Anything goes (inside a module section).
    Open,
    Unknown,
}

/// The settings schema as JSON, with just enough navigation for the checks
/// above: `$ref`s and `Option`'s `anyOf [T, null]` are followed.
struct SchemaView {
    root: Value,
}

impl SchemaView {
    fn new() -> Self {
        Self {
            root: Settings::json_schema().to_value(),
        }
    }

    fn root(&self) -> &Value {
        &self.root
    }

    fn resolve<'a>(&'a self, mut node: &'a Value) -> &'a Value {
        loop {
            if let Some(target) = node
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| r.strip_prefix('#'))
                .and_then(|pointer| self.root.pointer(pointer))
            {
                node = target;
                continue;
            }
            // `Option<T>` is `anyOf [T, null]`; other unions (enum variants
            // with descriptions) are left for the caller.
            if let Some([a, b]) = node
                .get("anyOf")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
            {
                let is_null = |m: &Value| m.get("type").and_then(Value::as_str) == Some("null");
                if is_null(b) {
                    node = a;
                    continue;
                }
                if is_null(a) {
                    node = b;
                    continue;
                }
            }
            return node;
        }
    }

    fn is_table(&self, node: &Value) -> bool {
        let node = self.resolve(node);
        schema_type(node) == Some("object") || node.get("properties").is_some()
    }

    fn child<'a>(&'a self, node: &'a Value, key: &str) -> Child<'a> {
        let node = self.resolve(node);
        if let Some(inner) = node.get("properties").and_then(|p| p.get(key)) {
            return Child::Known {
                schema: self.resolve(inner),
                named: true,
            };
        }
        // Maps with checked key names (`ai.providers`): the key itself is
        // checked when deserialising, with a better message than a pattern.
        if let Some(inner) = node
            .get("patternProperties")
            .and_then(Value::as_object)
            .and_then(|patterns| patterns.values().next())
        {
            return Child::Known {
                schema: self.resolve(inner),
                named: false,
            };
        }
        match node.get("additionalProperties") {
            Some(Value::Bool(false)) => Child::Unknown,
            Some(inner @ Value::Object(_)) => Child::Known {
                schema: self.resolve(inner),
                named: false,
            },
            _ => Child::Open,
        }
    }

    fn keys<'a>(&'a self, node: &'a Value) -> impl Iterator<Item = &'a str> {
        self.resolve(node)
            .get("properties")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|properties| properties.keys().map(String::as_str))
    }

    /// Every listed key, at any depth.
    fn all_keys(&self) -> Vec<KeyPath> {
        let mut out = Vec::new();
        let mut pending = vec![(KeyPath::default(), self.root())];
        while let Some((prefix, node)) = pending.pop() {
            for key in self.keys(node) {
                let path = prefix.child(key);
                if let Child::Known { schema, .. } = self.child(node, key) {
                    pending.push((path.clone(), schema));
                }
                out.push(path);
            }
        }
        out.sort();
        out
    }

    fn at(&self, key: &KeyPath) -> Option<&Value> {
        key.parts()
            .iter()
            .try_fold(self.root(), |node, part| match self.child(node, part) {
                Child::Known { schema, .. } => Some(schema),
                Child::Open | Child::Unknown => None,
            })
            .map(|node| self.resolve(node))
    }
}

#[cfg(test)]
mod tests {
    use priya_api::error::UserFacing;

    use super::*;
    use crate::settings::Layer;

    const HOME: &str = "/home/p";
    const USER_FILE: &str = "/home/p/.config/priya/settings.toml";

    /// The messages for a user settings file holding `text`.
    fn messages(text: &str) -> Vec<String> {
        let err = load(
            vec![Source::toml(Layer::User, USER_FILE, text)],
            Some(Path::new(HOME)),
        )
        .expect_err(text);
        err.0.iter().map(UserFacing::user_message).collect()
    }

    fn message(text: &str) -> String {
        let all = messages(text);
        assert_eq!(all.len(), 1, "{all:?}");
        all.into_iter().next().unwrap()
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    // regression: 0.6
    #[test]
    fn invalid_enum_value_gives_plain_message_with_location() {
        assert_eq!(
            message("# mine\n[ui]\ntheme = \"darkk\"\n"),
            "`ui.theme` must be light or dark; you wrote 'darkk' in ~/.config/priya/settings.toml line 3"
        );
        assert_eq!(
            message("[log]\nlevel = \"loud\"\n"),
            "`log.level` must be error, warn, info, debug or trace; you wrote 'loud' in ~/.config/priya/settings.toml line 2"
        );
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    // regression: 0.6
    #[test]
    fn unknown_core_key_rejected_with_suggestion() {
        // Every unknown key at once, each with its closest match.
        let all = messages("[ui]\nthemes = \"light\"\n[ai]\nmax_output_token = 5\ncolour = 1\n");
        assert_eq!(
            all,
            [
                "`ui.themes` isn't a setting; did you mean `ui.theme`? It is in ~/.config/priya/settings.toml line 2",
                "`ai.max_output_token` isn't a setting; did you mean `ai.max_output_tokens`? It is in ~/.config/priya/settings.toml line 4",
                "`ai.colour` isn't a setting; it is in ~/.config/priya/settings.toml line 5",
            ]
        );
        assert_eq!(
            message("[lgo]\nlevel = \"info\"\n"),
            "`lgo` isn't a setting; did you mean `log`? It is in ~/.config/priya/settings.toml line 1"
        );
        // Module sections are the module's business (0.8).
        assert!(
            load(
                vec![Source::toml(
                    Layer::User,
                    USER_FILE,
                    "[modules.weather]\nanything = 1\n"
                )],
                None
            )
            .is_ok()
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn wrong_types_and_ranges_say_what_is_expected() {
        assert_eq!(
            message("[ui.bar]\nwidth_px = 10\n"),
            "`ui.bar.width_px` must be a whole number from 320 to 7680; you wrote 10 in ~/.config/priya/settings.toml line 2"
        );
        assert_eq!(
            message("[ui.bar]\nwidth_px = \"wide\"\n"),
            "`ui.bar.width_px` must be a whole number from 320 to 7680; you wrote 'wide' in ~/.config/priya/settings.toml line 2"
        );
        assert_eq!(
            message("[ui]\nreduced_motion = \"yes\"\n"),
            "`ui.reduced_motion` must be true or false; you wrote 'yes' in ~/.config/priya/settings.toml line 2"
        );
        assert_eq!(
            message("[modules]\nenabled = \"weather\"\n"),
            "`modules.enabled` must be a list of text, like [\"a\", \"b\"]; you wrote 'weather' in ~/.config/priya/settings.toml line 2"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn refusals_explain_themselves() {
        assert_eq!(
            message("[ai]\nstrategy = \"cheapest\"\n"),
            "`ai.strategy`: that strategy arrives in Phase 1; for now, pick one model with fixed:<provider>/<model>; you wrote 'cheapest' in ~/.config/priya/settings.toml line 2"
        );
        assert_eq!(
            message("[ai.providers.ollama]\nbase_url = \"http://x\"\n"),
            "`ai.providers.ollama` needs a `kind`; add it in ~/.config/priya/settings.toml line 1"
        );
        let bad_name = message("[ai.providers.Ollama]\nkind = \"anthropic\"\n");
        assert!(
            bad_name.contains("'Ollama' can't be a provider name"),
            "{bad_name}"
        );
        assert!(
            bad_name.contains("line 1"),
            "the provider's header: {bad_name}"
        );
        // A misspelt `kind` is one problem, not two.
        assert_eq!(
            message("[ai.providers.x]\nkidn = \"anthropic\"\n"),
            "`ai.providers.x.kidn` isn't a setting; did you mean `ai.providers.x.kind`? It is in ~/.config/priya/settings.toml line 2"
        );
        // A table with an unrelated unknown key and no `kind` has both.
        assert_eq!(
            messages("[ai.providers.x]\nbrand = \"anthropic\"\n").len(),
            2
        );
        assert_eq!(
            message("[ai.providers.x]\nkind = \"openai\"\n"),
            "`ai.providers.x.kind` must be anthropic or openai_compatible; you wrote 'openai' in ~/.config/priya/settings.toml line 2"
        );
        // Nothing serde-shaped leaks through.
        for text in [
            "[ui]\ntheme = 3\n",
            "[ai]\nmax_output_tokens = 1.5\n",
            "[ai.providers.x]\nkind = \"openai\"\n",
        ] {
            let message = message(text);
            assert!(
                !message.contains("invalid type") && !message.contains("variant"),
                "{message}"
            );
        }
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_valid_load_keeps_every_value_and_its_origin() {
        let loaded = load(
            vec![Source::toml(
                Layer::User,
                USER_FILE,
                "[ai]\nstrategy = \"fixed:ollama/llama3.2:3b\"\n[ai.providers.ollama]\nkind = \"openai_compatible\"\n",
            )],
            None,
        )
        .unwrap();
        assert_eq!(
            loaded.settings.ai.strategy.unwrap().to_string(),
            "fixed:ollama/llama3.2:3b"
        );
        assert_eq!(
            loaded
                .layered
                .origin(&KeyPath::parse("ai.strategy").unwrap())
                .and_then(|o| o.line),
            Some(2)
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn every_invalid_value_is_reported_at_once_in_file_order() {
        assert_eq!(
            messages(
                "[ui]\ntheme = \"x\"\nreduced_motion = 3\n[ui.bar]\nwidth_px = 5\n[log]\nlevel = \"loud\"\n"
            ),
            [
                "`ui.theme` must be light or dark; you wrote 'x' in ~/.config/priya/settings.toml line 2",
                "`ui.reduced_motion` must be true or false; you wrote 3 in ~/.config/priya/settings.toml line 3",
                "`ui.bar.width_px` must be a whole number from 320 to 7680; you wrote 5 in ~/.config/priya/settings.toml line 5",
                "`log.level` must be error, warn, info, debug or trace; you wrote 'loud' in ~/.config/priya/settings.toml line 7",
            ]
        );
        // Unknown keys and bad values together.
        assert_eq!(messages("[ui]\nthemes = 1\nreduced_motion = 3\n").len(), 2);
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn module_sections_and_lists_get_plain_messages() {
        assert_eq!(
            message("[modules]\nenable = [\"weather\"]\n"),
            "`modules.enable` isn't a setting; did you mean `modules.enabled`? It is in ~/.config/priya/settings.toml line 2"
        );
        assert_eq!(
            message("[modules]\nweather = 5\n"),
            "`modules.weather` must be a section, like [modules.weather]; you wrote 5 in ~/.config/priya/settings.toml line 2"
        );
        assert_eq!(
            message("[modules]\nenabled = [1]\n"),
            "`modules.enabled` must be a list of text, like [\"a\", \"b\"]; see ~/.config/priya/settings.toml line 2"
        );
        assert_eq!(
            message("[ai.providers.x]\nkind = \"anthropic\"\nbase_url = 5\n"),
            "`ai.providers.x.base_url` must be text in quotes; you wrote 5 in ~/.config/priya/settings.toml line 3"
        );
        assert_eq!(
            message("ui = 5\n"),
            "`ui` must be a section, like [ui]; you wrote 5 in ~/.config/priya/settings.toml line 1"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn values_are_repeated_back_short_and_on_one_line() {
        let long = "x".repeat(300);
        let text = format!("[ui]\ntheme = \"{long}\"\n");
        let shown = message(&text);
        assert!(
            shown.contains(&format!("you wrote '{}…'", "x".repeat(40))),
            "{shown}"
        );
        let shown = message("[ui]\ntheme = \"\"\"\nlight\n\"\"\"\n");
        assert!(!shown.contains('\n'), "{shown}");
        assert!(shown.contains(r"you wrote 'light\n'"), "{shown}");
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn suggestions_look_in_other_sections_too() {
        assert_eq!(
            message("[ui]\nlevel = \"info\"\n"),
            "`ui.level` isn't a setting; did you mean `log.level`? It is in ~/.config/priya/settings.toml line 2"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn problems_are_listed_by_layer_then_file_then_line() {
        let err = load(
            vec![
                Source::overrides(serde_json::json!({"ui": {"colour": 1}})),
                Source::toml(Layer::User, USER_FILE, "[ui]\nshade = 1\n"),
                Source::toml(Layer::System, "/etc/priya/a.toml", "[ai]\nbrand = 1\n"),
            ],
            Some(Path::new(HOME)),
        )
        .unwrap_err();
        let keys: Vec<String> = err
            .0
            .iter()
            .map(|e| e.key.as_ref().unwrap().to_string())
            .collect();
        assert_eq!(keys, ["ai.brand", "ui.shade", "ui.colour"]);
        assert!(
            err.0[2].to_string().ends_with("given with this request"),
            "{}",
            err.0[2]
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_long_list_of_problems_says_there_are_more() {
        let text: String = (0..70)
            .map(|i| format!("[ai.providers.p{i}]\nkind = \"nope\"\n"))
            .collect();
        let all = messages(&text);
        assert_eq!(all.len(), 51, "{all:?}");
        assert_eq!(
            all.last().unwrap(),
            "Your settings: there are more problems; fix these first"
        );
        assert!(
            all[0].contains("must be anthropic or openai_compatible"),
            "{}",
            all[0]
        );
    }
}
