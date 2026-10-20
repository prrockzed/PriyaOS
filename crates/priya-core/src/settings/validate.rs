//! Checking the merged settings and turning them into [`Settings`].
//!
//! Unknown keys are found first, all of them, by walking the tree against
//! the JSON Schema; each gets a "did you mean" from the keys allowed there.
//! Then the tree is deserialised. A failure names the key, and the schema
//! for that key says what was expected ("light or dark", "a whole number
//! from 320 to 7680"), so messages never show serde's wording. Provenance
//! adds the file and line.

use std::path::Path;

use serde_json::{Map, Value};

use super::error::{Problem, SettingsError, SettingsErrors, shown};
use super::layers::{Layered, Source};
use super::model::Settings;

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
    let layered = Layered::merge(sources, home).map_err(SettingsErrors)?;
    let checker = Checker {
        schema: SchemaView::new(),
        layered: &layered,
        home,
    };
    let unknown = checker.unknown_keys();
    if !unknown.is_empty() {
        return Err(SettingsErrors(unknown));
    }
    let settings = checker.deserialize()?;
    Ok(Loaded { settings, layered })
}

struct Checker<'a> {
    schema: SchemaView,
    layered: &'a Layered,
    home: Option<&'a Path>,
}

impl Checker<'_> {
    fn error(&self, key: String, problem: Problem) -> SettingsError {
        let origin = self.layered.origin(&key);
        let file = origin.and_then(|o| o.file.clone());
        SettingsError {
            shown_file: file.as_deref().map(|f| shown(f, self.home)),
            file,
            line: origin.and_then(|o| o.line),
            key: Some(key),
            problem,
        }
    }

    fn unknown_keys(&self) -> Vec<SettingsError> {
        let mut found = Vec::new();
        self.walk(self.layered.tree(), self.schema.root(), "", &mut found);
        // In the order the owner will meet them in their files.
        found.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        found
    }

    fn walk(
        &self,
        tree: &Map<String, Value>,
        node: &Value,
        prefix: &str,
        found: &mut Vec<SettingsError>,
    ) {
        for (key, value) in tree {
            let path = join(prefix, key);
            match self.schema.child(node, key) {
                Child::Known(inner) => {
                    if let Value::Object(table) = value {
                        self.walk(table, inner, &path, found);
                    }
                }
                Child::Open => {}
                Child::Unknown => {
                    let suggestion =
                        closest(key, self.schema.keys(node)).map(|other| join(prefix, other));
                    found.push(self.error(path, Problem::UnknownKey { suggestion }));
                }
            }
        }
    }

    fn deserialize(&self) -> Result<Settings, SettingsErrors> {
        let tree = Value::Object(self.layered.tree().clone());
        serde_path_to_error::deserialize(&tree).map_err(|e| {
            let parts = key_parts(e.path());
            let message = e.into_inner().to_string();
            let found = value_at(&tree, &parts);
            let node = self.schema.at(&parts);
            SettingsErrors(vec![
                self.error(parts.join("."), problem(node, found, &message)),
            ])
        })
    }
}

/// What went wrong, in the schema's terms where it can say.
fn problem(node: Option<&Value>, found: Option<&Value>, message: &str) -> Problem {
    if let Some(field) = message
        .strip_prefix("missing field `")
        .and_then(|rest| rest.split('`').next())
    {
        return Problem::Missing {
            field: field.to_owned(),
        };
    }
    let found_text = found.and_then(show);
    let expected = node.and_then(|node| {
        expected_enum(node)
            .or_else(|| expected_range(node))
            .or_else(|| {
                found
                    .is_some_and(|v| !fits_type(node, v))
                    .then(|| expected_type(node))?
            })
    });
    match expected {
        Some(expected) => Problem::Invalid {
            expected,
            found: found_text,
        },
        // The value has the right shape but was refused for its meaning
        // (a strategy, a provider name): the type's own message says why.
        None => Problem::Rejected {
            message: message.to_owned(),
            found: found_text,
        },
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
    if node.get("type")?.as_str()? != "integer" {
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

fn expected_type(node: &Value) -> Option<String> {
    Some(
        match node.get("type")?.as_str()? {
            "boolean" => "true or false",
            "string" => "text in quotes",
            "array" => "a list, like [\"a\", \"b\"]",
            "object" => "a table",
            "number" => "a number",
            _ => return None,
        }
        .into(),
    )
}

/// Whether `value` has the JSON type the schema asks for.
fn fits_type(node: &Value, value: &Value) -> bool {
    match node.get("type").and_then(Value::as_str) {
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

/// A value as the owner wrote it; tables and lists aren't repeated back.
fn show(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(format!("'{s}'")),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// The map keys of a deserialisation path; a list index ends it, since
/// lists are single values in the settings tree.
fn key_parts(path: &serde_path_to_error::Path) -> Vec<String> {
    use serde_path_to_error::Segment;
    path.iter()
        .map_while(|segment| match segment {
            Segment::Map { key } => Some(Some(key.clone())),
            Segment::Enum { .. } | Segment::Unknown => Some(None),
            Segment::Seq { .. } => None,
        })
        .flatten()
        .collect()
}

fn value_at<'a>(tree: &'a Value, parts: &[String]) -> Option<&'a Value> {
    parts.iter().try_fold(tree, |node, part| node.get(part))
}

/// The allowed key nearest to `key`, if one is close enough to be a typo.
fn closest<'a>(key: &str, allowed: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (key.chars().count() / 3).max(1);
    allowed
        .map(|candidate| (strsim::damerau_levenshtein(key, candidate), candidate))
        .filter(|(distance, _)| *distance <= limit)
        .min()
        .map(|(_, candidate)| candidate)
}

fn join(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

/// What the schema allows under one key.
enum Child<'a> {
    Known(&'a Value),
    /// Anything goes (module sections).
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

    fn child<'a>(&'a self, node: &'a Value, key: &str) -> Child<'a> {
        let node = self.resolve(node);
        if let Some(inner) = node.get("properties").and_then(|p| p.get(key)) {
            return Child::Known(self.resolve(inner));
        }
        // Maps with checked key names (`ai.providers`): the key itself is
        // checked when deserialising, with a better message than a pattern.
        if let Some(inner) = node
            .get("patternProperties")
            .and_then(Value::as_object)
            .and_then(|patterns| patterns.values().next())
        {
            return Child::Known(self.resolve(inner));
        }
        match node.get("additionalProperties") {
            Some(Value::Bool(false)) => Child::Unknown,
            Some(inner @ Value::Object(_)) => Child::Known(self.resolve(inner)),
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

    fn at(&self, parts: &[String]) -> Option<&Value> {
        parts
            .iter()
            .try_fold(self.root(), |node, part| match self.child(node, part) {
                Child::Known(inner) => Some(inner),
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
            "`lgo` isn't a setting; did you mean `log`? It is in ~/.config/priya/settings.toml line 2"
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
            "`modules.enabled` must be a list, like [\"a\", \"b\"]; you wrote 'weather' in ~/.config/priya/settings.toml line 2"
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
            "`ai.providers.ollama` needs a `kind`; add it in ~/.config/priya/settings.toml line 2"
        );
        let bad_name = message("[ai.providers.Ollama]\nkind = \"anthropic\"\n");
        assert!(
            bad_name.contains("'Ollama' can't be a provider name"),
            "{bad_name}"
        );
        assert!(bad_name.contains("line 2"), "{bad_name}");
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
            loaded.layered.origin("ai.strategy").and_then(|o| o.line),
            Some(2)
        );
    }
}
