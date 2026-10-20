//! Changing a settings file in place (0.6).
//!
//! [`set`] and [`unset`] edit the file's text with `toml_edit`, so the
//! owner's comments, order and spacing survive: only the changed line
//! differs, and a replaced value keeps its trailing comment. Missing tables
//! are added as `[a.b]` sections. [`change`] does the same and then checks
//! the settings that would result, so a bad value never reaches the disk.
//! Writing the file is the caller's job.

use std::path::Path;

use priya_api::error::{ErrorKind, UserFacing};
use serde_json::{Map, Value};
use toml_edit::{DocumentMut, InlineTable, Item, Table};

use super::error::{SettingsErrors, capitalised};
use super::key::KeyPath;
use super::layers::{Content, Layer, Source};
use super::validate::{Loaded, load};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("the file isn't valid TOML, so it can't be changed safely: {0}")]
    Syntax(String),
    #[error("`{key}` can't be set: `{holder}` holds a value, not a section")]
    NotATable { key: KeyPath, holder: KeyPath },
    #[error("{0}")]
    Unrepresentable(String),
}

impl UserFacing for EditError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        capitalised(&self.to_string())
    }
}

/// `text` with `key` set to `value`.
pub fn set(text: &str, key: &KeyPath, value: &Value) -> Result<String, EditError> {
    let mut document = parse(text)?;
    let (last, parents) = split(key);
    let mut slot = Slot::Table(document.as_table_mut());
    for (i, part) in parents.iter().enumerate() {
        slot = slot.child(part).ok_or_else(|| EditError::NotATable {
            key: key.clone(),
            holder: KeyPath::from_parts(parents[..=i].iter().cloned()),
        })?;
    }
    slot.put(last, value)?;
    Ok(document.to_string())
}

/// `text` without `key`. A key that isn't there is fine: nothing changes.
pub fn unset(text: &str, key: &KeyPath) -> Result<String, EditError> {
    let mut document = parse(text)?;
    let (last, parents) = split(key);
    let mut slot = Slot::Table(document.as_table_mut());
    for part in parents {
        match slot.existing_child(part) {
            Some(child) => slot = child,
            None => return Ok(text.to_owned()),
        }
    }
    slot.remove(last);
    Ok(document.to_string())
}

/// A change to one settings key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change<'a> {
    Set(&'a KeyPath, &'a Value),
    Unset(&'a KeyPath),
}

/// Why a change was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChangeError {
    #[error(transparent)]
    Edit(#[from] EditError),
    /// The settings that would result don't pass their checks.
    #[error(transparent)]
    Invalid(#[from] SettingsErrors),
}

impl UserFacing for ChangeError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        match self {
            Self::Edit(e) => e.user_message(),
            Self::Invalid(e) => e.user_message(),
        }
    }
}

/// The new text of `target` after `change`, and the settings it would give
/// together with the other `sources`. `target` is one of the sources, or a
/// user-layer file that doesn't exist yet. Nothing is written.
pub fn change(
    sources: &[Source],
    target: &Path,
    change: Change<'_>,
    home: Option<&Path>,
) -> Result<(String, Loaded), ChangeError> {
    let current = sources
        .iter()
        .find(|s| s.file.as_deref() == Some(target))
        .map(|s| match &s.content {
            Content::Toml(text) => (s.layer, text.as_str()),
            Content::Overrides(_) => (s.layer, ""),
        });
    let (layer, text) = current.unwrap_or((Layer::User, ""));
    let new_text = match change {
        Change::Set(key, value) => set(text, key, value)?,
        Change::Unset(key) => unset(text, key)?,
    };
    let others = sources
        .iter()
        .filter(|s| s.file.as_deref() != Some(target))
        .cloned();
    let edited = Source::toml(layer, target, new_text.clone());
    let loaded = load(others.chain([edited]), home)?;
    Ok((new_text, loaded))
}

fn parse(text: &str) -> Result<DocumentMut, EditError> {
    text.parse()
        .map_err(|e: toml_edit::TomlError| EditError::Syntax(e.message().to_owned()))
}

/// The last part and the parts before it. A `KeyPath` is never empty when
/// parsed; an empty one made by hand edits the top level.
fn split(key: &KeyPath) -> (&str, &[String]) {
    match key.parts().split_last() {
        Some((last, parents)) => (last, parents),
        None => ("", &[]),
    }
}

/// A table being walked: a `[section]` (or dotted key) or an inline table.
enum Slot<'a> {
    Table(&'a mut Table),
    Inline(&'a mut InlineTable),
}

impl<'a> Slot<'a> {
    /// The table under `part`, made if missing; `None` if a value is there.
    fn child(self, part: &str) -> Option<Self> {
        match self {
            Self::Table(table) => {
                let item = table.entry(part).or_insert_with(|| {
                    // Implicit: only the deepest new table gets a header.
                    let mut new = Table::new();
                    new.set_implicit(true);
                    Item::Table(new)
                });
                Self::from_item(item)
            }
            Self::Inline(table) => match table
                .entry(part)
                .or_insert(toml_edit::Value::InlineTable(InlineTable::new()))
            {
                toml_edit::Value::InlineTable(inner) => Some(Self::Inline(inner)),
                _ => None,
            },
        }
    }

    /// The table under `part`, if it exists.
    fn existing_child(self, part: &str) -> Option<Self> {
        match self {
            Self::Table(table) => table.get_mut(part).and_then(Self::from_item),
            Self::Inline(table) => match table.get_mut(part)? {
                toml_edit::Value::InlineTable(inner) => Some(Self::Inline(inner)),
                _ => None,
            },
        }
    }

    fn from_item(item: &'a mut Item) -> Option<Self> {
        match item {
            Item::Table(table) => Some(Self::Table(table)),
            Item::Value(toml_edit::Value::InlineTable(table)) => Some(Self::Inline(table)),
            _ => None,
        }
    }

    /// Sets `key` here. A value that replaces a value keeps its spacing and
    /// trailing comment.
    fn put(self, key: &str, value: &Value) -> Result<(), EditError> {
        match self {
            Self::Table(table) => {
                if let (Some(Item::Value(old)), false) = (table.get_mut(key), value.is_object()) {
                    replace_keeping_decor(old, to_toml(value)?);
                } else {
                    table.insert(key, to_item(value)?);
                }
            }
            Self::Inline(table) => match table.get_mut(key) {
                Some(old) => replace_keeping_decor(old, to_toml(value)?),
                None => {
                    // The space before `}` belongs to the last value; move
                    // it to the new last one, so `{ a = 1 }` grows into
                    // `{ a = 1, b = 2 }`.
                    let mut new = to_toml(value)?;
                    if let Some((_, last)) = table.iter_mut().last()
                        && let Some(trailing) = last.decor().suffix().cloned()
                    {
                        last.decor_mut().set_suffix("");
                        new.decor_mut().set_suffix(trailing);
                    }
                    table.insert(key, new);
                }
            },
        }
        Ok(())
    }

    fn remove(self, key: &str) {
        match self {
            Self::Table(table) => {
                table.remove(key);
            }
            Self::Inline(table) => {
                table.remove(key);
            }
        }
    }
}

fn replace_keeping_decor(old: &mut toml_edit::Value, new: toml_edit::Value) {
    let decor = old.decor().clone();
    *old = new;
    *old.decor_mut() = decor;
}

/// A JSON value for a table slot: objects become `[sections]`.
fn to_item(value: &Value) -> Result<Item, EditError> {
    match value {
        Value::Object(map) => {
            let mut table = Table::new();
            for (key, inner) in map {
                table.insert(key, to_item(inner)?);
            }
            Ok(Item::Table(table))
        }
        _ => Ok(Item::Value(to_toml(value)?)),
    }
}

/// A JSON value as a TOML value: objects become inline tables.
fn to_toml(value: &Value) -> Result<toml_edit::Value, EditError> {
    Ok(match value {
        Value::Null => {
            return Err(EditError::Unrepresentable(
                "a setting can't be empty; unset it instead".into(),
            ));
        }
        Value::Bool(b) => (*b).into(),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.into(),
            (None, Some(f)) if n.is_f64() => f.into(),
            _ => {
                return Err(EditError::Unrepresentable(format!(
                    "{n} is too large for a settings file"
                )));
            }
        },
        Value::String(s) => s.as_str().into(),
        Value::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(to_toml(item)?);
            }
            toml_edit::Value::Array(array)
        }
        Value::Object(map) => toml_edit::Value::InlineTable(inline(map)?),
    })
}

fn inline(map: &Map<String, Value>) -> Result<InlineTable, EditError> {
    let mut table = InlineTable::new();
    for (key, value) in map {
        table.insert(key, to_toml(value)?);
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::settings::Theme;

    fn k(text: &str) -> KeyPath {
        KeyPath::parse(text).unwrap()
    }

    /// A file as the owner might keep it: comments everywhere, odd spacing,
    /// keys out of the usual order.
    const MINE: &str = r#"# My PriyaOS settings.
# Last tidied in October.

settings_version = 1

[ai]
max_output_tokens = 4000     # long answers, please
# strategy = "fixed:anthropic/claude-sonnet-5-5"

[ui]
# I like it dark at night.
theme   = "dark"   # light or dark
reduced_motion = true

[ui.bar]
width_px = 900

[modules]
enabled = ["weather"]  # just this one
"#;

    /// The lines of `after` that differ from `before`, when both have the
    /// same number of lines.
    fn changed_lines(before: &str, after: &str) -> Vec<(String, String)> {
        assert_eq!(before.lines().count(), after.lines().count(), "{after}");
        before
            .lines()
            .zip(after.lines())
            .filter(|(a, b)| a != b)
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect()
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn set_preserves_comments_and_order() {
        let after = set(MINE, &k("ui.theme"), &json!("light")).unwrap();
        assert_eq!(
            changed_lines(MINE, &after),
            [(
                r#"theme   = "dark"   # light or dark"#.to_owned(),
                r#"theme   = "light"   # light or dark"#.to_owned()
            )]
        );
        let after = set(MINE, &k("ai.max_output_tokens"), &json!(2048)).unwrap();
        assert_eq!(
            changed_lines(MINE, &after),
            [(
                "max_output_tokens = 4000     # long answers, please".to_owned(),
                "max_output_tokens = 2048     # long answers, please".to_owned()
            )]
        );
        let after = set(MINE, &k("modules.enabled"), &json!(["weather", "clock"])).unwrap();
        assert_eq!(changed_lines(MINE, &after).len(), 1);
        assert!(
            after.contains(r#"enabled = ["weather", "clock"]  # just this one"#),
            "{after}"
        );
        // Setting the value it already has changes nothing at all.
        assert_eq!(set(MINE, &k("ui.bar.width_px"), &json!(900)).unwrap(), MINE);
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn set_adds_missing_keys_and_sections() {
        assert_eq!(
            set("", &k("ui.bar.width_px"), &json!(800)).unwrap(),
            "[ui.bar]\nwidth_px = 800\n"
        );
        let after = set(MINE, &k("ui.bar.height_px"), &json!(40)).unwrap();
        assert!(
            after.ends_with("enabled = [\"weather\"]  # just this one\n"),
            "only adds inside [ui.bar]: {after}"
        );
        assert!(
            after.contains("width_px = 900\nheight_px = 40\n"),
            "{after}"
        );
        let after = set(
            MINE,
            &k("ai.providers.ollama"),
            &json!({"kind": "openai_compatible"}),
        )
        .unwrap();
        assert!(
            after.contains("[ai.providers.ollama]\nkind = \"openai_compatible\"\n"),
            "{after}"
        );
        // Dotted keys and inline tables are walked as tables.
        assert_eq!(
            set(
                "[ai]\nproviders.ollama.kind = \"x\"\n",
                &k("ai.providers.ollama.kind"),
                &json!("openai_compatible")
            )
            .unwrap(),
            "[ai]\nproviders.ollama.kind = \"openai_compatible\"\n"
        );
        assert_eq!(
            set(
                "[ai.providers]\nx = { kind = \"a\" }\n",
                &k("ai.providers.x.base_url"),
                &json!("http://h")
            )
            .unwrap(),
            "[ai.providers]\nx = { kind = \"a\", base_url = \"http://h\" }\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn quoted_keys_are_written_as_one_key() {
        let key = k(r#"ai.providers."my.lab".kind"#);
        assert_eq!(
            set("", &key, &json!("anthropic")).unwrap(),
            "[ai.providers.\"my.lab\"]\nkind = \"anthropic\"\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn unset_removes_only_that_key() {
        let after = unset(MINE, &k("ui.reduced_motion")).unwrap();
        assert_eq!(after, MINE.replace("reduced_motion = true\n", ""));
        assert_eq!(unset(MINE, &k("ui.nothing.here")).unwrap(), MINE);
        assert_eq!(unset(MINE, &k("nothing")).unwrap(), MINE);
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn edits_that_cant_be_written_are_refused() {
        assert!(matches!(
            set(MINE, &k("ui.theme.dark"), &json!(1)),
            Err(EditError::NotATable { holder, .. }) if holder == k("ui.theme")
        ));
        assert!(matches!(
            set(MINE, &k("ui.theme"), &Value::Null),
            Err(EditError::Unrepresentable(_))
        ));
        assert!(matches!(
            set(MINE, &k("ui.bar.width_px"), &json!(u64::MAX)),
            Err(EditError::Unrepresentable(_))
        ));
        assert!(matches!(
            set("[ui\n", &k("ui.theme"), &json!("light")),
            Err(EditError::Syntax(_))
        ));
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn set_rejects_invalid_value_without_touching_file() {
        let path = Path::new("/home/p/.config/priya/settings.toml");
        let sources = vec![Source::toml(Layer::User, path, MINE)];
        let home = Some(Path::new("/home/p"));

        let err = change(
            &sources,
            path,
            Change::Set(&k("ui.theme"), &json!("darkk")),
            home,
        )
        .unwrap_err();
        assert_eq!(
            err.user_message(),
            "`ui.theme` must be light or dark; you wrote 'darkk' in ~/.config/priya/settings.toml line 12"
        );
        let err = change(
            &sources,
            path,
            Change::Set(&k("ui.themes"), &json!("light")),
            home,
        )
        .unwrap_err();
        assert!(
            err.user_message().contains("did you mean `ui.theme`"),
            "{err}"
        );

        let (text, loaded) = change(
            &sources,
            path,
            Change::Set(&k("ui.theme"), &json!("light")),
            home,
        )
        .unwrap();
        assert_eq!(loaded.settings.ui.theme, Theme::Light);
        assert_eq!(changed_lines(MINE, &text).len(), 1);
        // A file that doesn't exist yet starts empty, in the user layer.
        let new = Path::new("/home/p/.config/priya/extra.toml");
        let (text, loaded) = change(&sources, new, Change::Unset(&k("ui.theme")), home).unwrap();
        assert_eq!(text, "");
        assert_eq!(loaded.settings.ui.theme, Theme::Dark);
    }
}
