//! Changing a settings file in place (0.6).
//!
//! [`set`] and [`unset`] edit the file's text with `toml_edit`, so the
//! owner's comments, order, spacing and line endings survive: only the
//! changed lines differ.
//! - A replaced value keeps its trailing comment.
//! - A table or inline table given a new set of keys is edited in place.
//! - A new section goes at the end of the file, so it never captures the
//!   comments of the section after it.
//! - `unset` removes a key with the comments written right above it, but
//!   keeps a section's comments (those before a blank line). A section left
//!   empty keeps its header.
//!
//! [`change`] edits one file and checks it, refusing the change only for
//! the problems it adds. Writing the file is the caller's job.

use std::path::Path;

use priya_api::error::{ErrorKind, UserFacing};
use serde_json::{Map, Value};
use toml_edit::{DocumentMut, InlineTable, Item, Table};

use super::error::{SettingsError, SettingsErrors, capitalised};
use super::key::KeyPath;
use super::layers::{Content, Layer, Source};
use super::migrate::{Migrated, migrate};
use super::validate::load;

/// Floats this large or small (but not zero) are written with an exponent.
const PLAIN_FLOAT: std::ops::Range<f64> = 1e-4..1e16;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("the file isn't valid TOML: {0}")]
    Syntax(String),
    #[error("a setting needs a key")]
    NoKey,
    #[error("`{key}` can't be set: `{holder}` holds a value, not a section")]
    NotATable { key: KeyPath, holder: KeyPath },
    #[error("`{key}` can't be set: `{holder}` is a list of sections")]
    InList { key: KeyPath, holder: KeyPath },
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
    let (last, parents) = split(key)?;
    let style = Style::of(text);
    let mut document = parse(text)?;
    let mut positions = Positions::after(&document);
    let mut slot = Slot::Table(document.as_table_mut());
    for (i, part) in parents.iter().enumerate() {
        let holder = || KeyPath::from_parts(parents[..=i].iter().cloned());
        slot = slot
            .child(part, &mut positions)
            .map_err(|blocker| match blocker {
                Blocker::Value => EditError::NotATable {
                    key: key.clone(),
                    holder: holder(),
                },
                Blocker::List => EditError::InList {
                    key: key.clone(),
                    holder: holder(),
                },
            })?;
    }
    slot.put(last, value, &mut positions)?;
    Ok(style.restore(document.to_string()))
}

/// `text` without `key`. A key that isn't there is fine: nothing changes.
pub fn unset(text: &str, key: &KeyPath) -> Result<String, EditError> {
    let (last, parents) = split(key)?;
    let style = Style::of(text);
    let mut document = parse(text)?;
    let mut slot = Slot::Table(document.as_table_mut());
    for part in parents {
        match slot.existing_child(part) {
            Some(child) => slot = child,
            None => return Ok(text.to_owned()),
        }
    }
    let after = slot.position();
    if let Some(comments) = slot.remove(last) {
        // The section's own comments came right before the next section.
        give_to_next_section(&mut document, after, &comments);
    }
    Ok(style.restore(document.to_string()))
}

/// A change to one settings key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change<'a> {
    Set(&'a KeyPath, &'a Value),
    Unset(&'a KeyPath),
}

/// The file a change goes to, and its layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target<'a> {
    pub layer: Layer,
    pub file: &'a Path,
}

/// Why a change was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChangeError {
    #[error(transparent)]
    Edit(#[from] EditError),
    /// The file as it is can't be read, so it can't be edited safely.
    #[error(transparent)]
    Unreadable(SettingsErrors),
    /// The change would add these problems.
    #[error("not changed: {}", summaries(.0))]
    Refused(Vec<SettingsError>),
}

impl UserFacing for ChangeError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        match self {
            Self::Edit(e) => e.user_message(),
            Self::Unreadable(e) => e.user_message(),
            Self::Refused(_) => capitalised(&self.to_string()),
        }
    }
}

fn summaries(errors: &[SettingsError]) -> String {
    errors
        .iter()
        .map(SettingsError::summary)
        .collect::<Vec<_>>()
        .join("; ")
}

/// The new text of `target` after `change`. `target` may be one of the
/// `sources` or a file that doesn't exist yet. An older-format file is
/// upgraded as part of the edit.
///
/// The edited file is checked with the layers up to its own, so a higher
/// layer setting the same key can't hide a bad value. Only problems the
/// change adds refuse it, so a file with problems elsewhere can still be
/// fixed one key at a time. Nothing is written.
pub fn change(
    sources: &[Source],
    target: Target<'_>,
    change: Change<'_>,
    home: Option<&Path>,
) -> Result<String, ChangeError> {
    let current = sources
        .iter()
        .find(|s| s.file.as_deref() == Some(target.file))
        .and_then(|s| match &s.content {
            Content::Toml(text) => Some(text.as_str()),
            Content::Overrides(_) => None,
        })
        .unwrap_or("");
    let as_source = |text: &str| Source::toml(target.layer, target.file, text);
    let base = match migrate(current) {
        Ok(Migrated::Current) => current.to_owned(),
        Ok(Migrated::Upgraded { text, .. }) => text,
        // The loader says what's wrong, with the line.
        Err(_) => {
            let errors = load([as_source(current)], home)
                .err()
                .unwrap_or(SettingsErrors(Vec::new()));
            return Err(ChangeError::Unreadable(errors));
        }
    };
    let edited = match change {
        Change::Set(key, value) => set(&base, key, value)?,
        Change::Unset(key) => unset(&base, key)?,
    };

    let below: Vec<Source> = sources
        .iter()
        .filter(|s| s.layer <= target.layer && s.file.as_deref() != Some(target.file))
        .cloned()
        .collect();
    let problems = |text: &str| {
        load(below.iter().cloned().chain([as_source(text)]), home)
            .err()
            .map(|errors| errors.0)
            .unwrap_or_default()
    };
    let before = problems(current);
    let added: Vec<SettingsError> = problems(&edited)
        .into_iter()
        .filter(|after| {
            !before
                .iter()
                .any(|b| b.key == after.key && b.problem == after.problem)
        })
        .collect();
    if added.is_empty() {
        Ok(edited)
    } else {
        Err(ChangeError::Refused(added))
    }
}

fn parse(text: &str) -> Result<DocumentMut, EditError> {
    text.parse()
        .map_err(|e: toml_edit::TomlError| EditError::Syntax(e.message().to_owned()))
}

fn split(key: &KeyPath) -> Result<(&str, &[String]), EditError> {
    let (last, parents) = key.parts().split_last().ok_or(EditError::NoKey)?;
    Ok((last, parents))
}

/// The file's line endings and whether it ends with one, which `toml_edit`
/// doesn't keep.
struct Style {
    crlf: bool,
    final_newline: bool,
}

impl Style {
    fn of(text: &str) -> Self {
        Self {
            crlf: text.contains("\r\n"),
            final_newline: text.is_empty() || text.ends_with('\n'),
        }
    }

    fn restore(&self, text: String) -> String {
        let mut text = text.replace("\r\n", "\n");
        if !self.final_newline && text.ends_with('\n') {
            text.pop();
        }
        if self.crlf {
            text = text.replace('\n', "\r\n");
        }
        text
    }
}

/// Positions for new `[sections]`: after every existing one, so they go at
/// the end of the file.
struct Positions(isize);

impl Positions {
    fn after(document: &DocumentMut) -> Self {
        fn highest(table: &Table) -> isize {
            let mut top = table.position().unwrap_or(0);
            for (_, item) in table.iter() {
                match item {
                    Item::Table(inner) => top = top.max(highest(inner)),
                    Item::ArrayOfTables(tables) => {
                        for inner in tables.iter() {
                            top = top.max(highest(inner));
                        }
                    }
                    _ => {}
                }
            }
            top
        }
        Self(highest(document.as_table()))
    }

    fn next(&mut self) -> isize {
        self.0 += 1;
        self.0
    }
}

/// Why a key can't be walked into.
enum Blocker {
    Value,
    List,
}

/// A table being walked: a `[section]` (or dotted key) or an inline table.
enum Slot<'a> {
    Table(&'a mut Table),
    Inline(&'a mut InlineTable),
}

impl<'a> Slot<'a> {
    /// The table under `part`, made if missing.
    fn child(self, part: &str, positions: &mut Positions) -> Result<Self, Blocker> {
        match self {
            Self::Table(table) => {
                let item = table.entry(part).or_insert_with(|| {
                    // Implicit: only a table holding values gets a header.
                    let mut new = Table::new();
                    new.set_implicit(true);
                    new.set_position(Some(positions.next()));
                    Item::Table(new)
                });
                match item {
                    Item::ArrayOfTables(_) => Err(Blocker::List),
                    other => Self::from_item(other).ok_or(Blocker::Value),
                }
            }
            Self::Inline(table) => match table
                .entry(part)
                .or_insert(toml_edit::Value::InlineTable(InlineTable::new()))
            {
                toml_edit::Value::InlineTable(inner) => Ok(Self::Inline(inner)),
                _ => Err(Blocker::Value),
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

    /// Where this table's header is, for finding the section after it.
    fn position(&self) -> Option<isize> {
        match self {
            Self::Table(table) => table.position(),
            Self::Inline(_) => None,
        }
    }

    /// Sets `key` here, in place where possible.
    fn put(self, key: &str, value: &Value, positions: &mut Positions) -> Result<(), EditError> {
        match self {
            Self::Table(table) => match (table.get_mut(key), value) {
                // A value (inline tables too) is replaced keeping its
                // spacing and comment.
                (Some(Item::Value(old)), _) => {
                    replace_keeping_decor(old, to_toml(value, true)?);
                }
                // A section given new keys is edited key by key, so
                // unchanged lines keep their comments.
                (Some(Item::Table(old)), Value::Object(map)) => {
                    let stale: Vec<String> = old
                        .iter()
                        .map(|(k, _)| k.to_owned())
                        .filter(|k| !map.contains_key(k))
                        .collect();
                    for gone in stale {
                        remove_from_table(old, &gone);
                    }
                    for (inner_key, inner) in map {
                        Slot::Table(&mut *old).put(inner_key, inner, positions)?;
                    }
                }
                (_, Value::Object(map)) => {
                    table.insert(key, Item::Table(new_table(map, positions)?));
                }
                (_, _) => {
                    table.insert(key, Item::Value(to_toml(value, true)?));
                }
            },
            Self::Inline(table) => match table.get_mut(key) {
                Some(old) => replace_keeping_decor(old, to_toml(value, true)?),
                None => {
                    // The space before `}` belongs to the last value; move
                    // it to the new last one, so `{ a = 1 }` grows into
                    // `{ a = 1, b = 2 }`.
                    let mut new = to_toml(value, true)?;
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

    /// Removes `key`. Returns the section comments it carried when there is
    /// no later key here to give them to.
    fn remove(self, key: &str) -> Option<String> {
        match self {
            Self::Table(table) => remove_from_table(table, key),
            Self::Inline(table) => {
                let was_last = table.iter().last().is_some_and(|(k, _)| k == key);
                let removed = table.remove(key);
                // Give the space before `}` back to the new last value.
                if was_last
                    && let Some(trailing) =
                        removed.as_ref().and_then(|v| v.decor().suffix().cloned())
                    && let Some((_, last)) = table.iter_mut().last()
                {
                    last.decor_mut().set_suffix(trailing);
                }
                None
            }
        }
    }
}

/// Removes `key` from a section with the comments written right above it.
/// Comments before a blank line belong to the section: they move to the
/// next key, or are returned when there is none.
fn remove_from_table(table: &mut Table, key: &str) -> Option<String> {
    let prefix = table
        .key(key)
        .and_then(|k| k.leaf_decor().prefix())
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .to_owned();
    let next = table
        .iter()
        .map(|(k, _)| k.to_owned())
        .skip_while(|k| k != key)
        .nth(1);
    table.remove(key)?;
    let kept = section_comments(&prefix);
    if kept.is_empty() {
        return None;
    }
    match next.and_then(|next| table.key_mut(&next)) {
        Some(mut next) => {
            let decor = next.leaf_decor_mut();
            let old = decor
                .prefix()
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_owned();
            decor.set_prefix(format!("{kept}{old}"));
            None
        }
        None => Some(kept.to_owned()),
    }
}

/// The part of a key's leading comments up to its last blank line.
fn section_comments(prefix: &str) -> &str {
    let mut end = 0;
    let mut at = 0;
    for line in prefix.split_inclusive('\n') {
        at += line.len();
        if line.trim().is_empty() {
            end = at;
        }
    }
    &prefix[..end]
}

/// Puts `comments` before the first section after position `after` (the
/// top of the file when `None`), or at the end of the file.
fn give_to_next_section(document: &mut DocumentMut, after: Option<isize>, comments: &str) {
    fn positions(table: &Table, out: &mut Vec<isize>) {
        for (_, item) in table.iter() {
            let tables: Vec<&Table> = match item {
                Item::Table(inner) => vec![inner],
                Item::ArrayOfTables(array) => array.iter().collect(),
                _ => Vec::new(),
            };
            for inner in tables {
                if !inner.is_implicit() {
                    out.extend(inner.position());
                }
                positions(inner, out);
            }
        }
    }
    fn find(table: &mut Table, wanted: isize) -> Option<&mut Table> {
        for (_, item) in table.iter_mut() {
            let tables: Vec<&mut Table> = match item {
                Item::Table(inner) => vec![inner],
                Item::ArrayOfTables(array) => array.iter_mut().collect(),
                _ => Vec::new(),
            };
            for inner in tables {
                if inner.position() == Some(wanted) && !inner.is_implicit() {
                    return Some(inner);
                }
                if let Some(found) = find(inner, wanted) {
                    return Some(found);
                }
            }
        }
        None
    }
    let mut all = Vec::new();
    positions(document.as_table(), &mut all);
    let next = all
        .into_iter()
        .filter(|p| after.is_none_or(|after| *p > after))
        .min();
    match next.and_then(|p| find(document.as_table_mut(), p)) {
        Some(section) => {
            let decor = section.decor_mut();
            let old = decor
                .prefix()
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_owned();
            decor.set_prefix(format!("{comments}{old}"));
        }
        None => {
            let old = document.trailing().as_str().unwrap_or("").to_owned();
            document.set_trailing(format!("{comments}{old}"));
        }
    }
}

fn replace_keeping_decor(old: &mut toml_edit::Value, new: toml_edit::Value) {
    let decor = old.decor().clone();
    *old = new;
    *old.decor_mut() = decor;
}

/// A new `[section]` holding `map`, placed after every existing one.
fn new_table(map: &Map<String, Value>, positions: &mut Positions) -> Result<Table, EditError> {
    let mut table = Table::new();
    table.set_position(Some(positions.next()));
    for (key, value) in map {
        let item = match value {
            Value::Object(inner) => Item::Table(new_table(inner, positions)?),
            _ => Item::Value(to_toml(value, true)?),
        };
        table.insert(key, item);
    }
    Ok(table)
}

/// A JSON value as a TOML value: objects become inline tables. `top` is
/// false inside a list or table, for the message about empty values.
fn to_toml(value: &Value, top: bool) -> Result<toml_edit::Value, EditError> {
    Ok(match value {
        Value::Null if top => {
            return Err(EditError::Unrepresentable(
                "a setting can't be empty; unset it instead".into(),
            ));
        }
        Value::Null => {
            return Err(EditError::Unrepresentable(
                "a list or table can't contain an empty value".into(),
            ));
        }
        Value::Bool(b) => (*b).into(),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.into(),
            (None, Some(f)) if n.is_f64() => float(f),
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
                array.push(to_toml(item, false)?);
            }
            toml_edit::Value::Array(array)
        }
        Value::Object(map) => {
            let mut table = InlineTable::new();
            for (key, value) in map {
                table.insert(key, to_toml(value, false)?);
            }
            toml_edit::Value::InlineTable(table)
        }
    })
}

/// A float, with an exponent when it would otherwise be very long.
fn float(f: f64) -> toml_edit::Value {
    if f == 0.0 || PLAIN_FLOAT.contains(&f.abs()) {
        return f.into();
    }
    format!("{f:e}")
        .parse()
        .unwrap_or_else(|_| toml_edit::Value::from(f))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
            set("[[jobs]]\nn = 1\n", &k("jobs.n"), &json!(2)),
            Err(EditError::InList { .. })
        ));
        assert_eq!(
            set(MINE, &KeyPath::default(), &json!(1)),
            Err(EditError::NoKey)
        );
        assert_eq!(
            set(MINE, &k("modules.enabled"), &json!(["a", null]))
                .unwrap_err()
                .to_string(),
            "a list or table can't contain an empty value"
        );
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

    const USER: &str = "/home/p/.config/priya/settings.toml";
    fn home() -> Option<&'static Path> {
        Some(Path::new("/home/p"))
    }

    fn user() -> Target<'static> {
        Target {
            layer: Layer::User,
            file: Path::new(USER),
        }
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn set_rejects_invalid_value_without_touching_file() {
        // The pure part: nothing to write is returned. priyad's service test
        // checks the file on disk is left alone.
        let sources = vec![Source::toml(Layer::User, USER, MINE)];
        let err = change(
            &sources,
            user(),
            Change::Set(&k("ui.theme"), &json!("darkk")),
            home(),
        )
        .unwrap_err();
        assert_eq!(
            err.user_message(),
            "Not changed: `ui.theme` must be light or dark (you gave 'darkk')"
        );
        let err = change(
            &sources,
            user(),
            Change::Set(&k("ui.themes"), &json!("light")),
            home(),
        )
        .unwrap_err();
        assert_eq!(
            err.user_message(),
            "Not changed: `ui.themes` isn't a setting; did you mean `ui.theme`?"
        );

        let text = change(
            &sources,
            user(),
            Change::Set(&k("ui.theme"), &json!("light")),
            home(),
        )
        .unwrap();
        assert_eq!(changed_lines(MINE, &text).len(), 1);
        // A file that doesn't exist yet starts empty.
        let extra = Target {
            layer: Layer::User,
            file: Path::new("/home/p/.config/priya/extra.toml"),
        };
        let text = change(
            &sources,
            extra,
            Change::Set(&k("ui.theme"), &json!("light")),
            home(),
        )
        .unwrap();
        assert_eq!(text, "[ui]\ntheme = \"light\"\n");
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_higher_layer_cant_hide_a_bad_value() {
        let sources = vec![
            Source::toml(Layer::User, USER, ""),
            Source::toml(
                Layer::Project,
                "/home/p/code/.priya.toml",
                "[ui]\ntheme = \"light\"\n",
            ),
            Source::overrides(json!({"ui": {"theme": "light"}})),
        ];
        let err = change(
            &sources,
            user(),
            Change::Set(&k("ui.theme"), &json!("darkk")),
            home(),
        )
        .unwrap_err();
        assert!(
            err.user_message().contains("must be light or dark"),
            "{err}"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_file_with_problems_can_be_fixed_one_key_at_a_time() {
        let broken = "[ui]\ntheme = \"darkk\"\nreduced_motion = \"yes\"\n";
        let sources = vec![
            Source::toml(
                Layer::System,
                "/etc/priya/a.toml",
                "[log]\nlevel = \"loud\"\n",
            ),
            Source::toml(Layer::User, USER, broken),
        ];
        // Neither the other bad key nor the broken /etc file blocks it.
        let fixed = change(
            &sources,
            user(),
            Change::Set(&k("ui.theme"), &json!("dark")),
            home(),
        )
        .unwrap();
        assert_eq!(fixed, "[ui]\ntheme = \"dark\"\nreduced_motion = \"yes\"\n");
        // But a new problem still does.
        assert!(
            change(
                &sources,
                user(),
                Change::Set(&k("ui.bar.width_px"), &json!(1)),
                home()
            )
            .is_err()
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn an_unreadable_file_is_reported_with_its_line_not_edited() {
        let sources = vec![Source::toml(Layer::User, USER, "[ui]\ntheme = dark\n")];
        let err = change(
            &sources,
            user(),
            Change::Set(&k("ui.theme"), &json!("light")),
            home(),
        )
        .unwrap_err();
        assert_eq!(
            err.user_message(),
            "~/.config/priya/settings.toml line 2 isn't valid TOML: text needs quotes, like theme = \"dark\""
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn line_endings_and_a_missing_final_newline_are_kept() {
        let crlf = MINE.replace('\n', "\r\n");
        let after = set(&crlf, &k("ui.theme"), &json!("light")).unwrap();
        assert_eq!(
            after,
            crlf.replace("theme   = \"dark\"", "theme   = \"light\"")
        );
        assert_eq!(unset(&crlf, &k("nothing.here")).unwrap(), crlf);
        assert_eq!(
            set("[ui]\ntheme = \"dark\"", &k("ui.theme"), &json!("light")).unwrap(),
            "[ui]\ntheme = \"light\""
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn new_sections_go_at_the_end_of_the_file() {
        let text =
            "[ai]\nmax_output_tokens = 1\n# strategy = \"fixed:x/y\"\n\n[ui]\ntheme = \"dark\"\n";
        let after = set(
            text,
            &k("ai.providers.ollama"),
            &json!({"kind": "openai_compatible"}),
        )
        .unwrap();
        assert_eq!(
            after,
            format!("{text}\n[ai.providers.ollama]\nkind = \"openai_compatible\"\n"),
            "the commented line stays in [ai]"
        );
        let after = set(
            "# my file\n[log]\nlevel = \"info\"\n",
            &k("ui.theme"),
            &json!("light"),
        )
        .unwrap();
        assert!(after.starts_with("# my file\n[log]\n"), "{after}");
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn tables_given_new_keys_are_edited_in_place() {
        let inline = "[ai.providers]\nx = { kind = \"a\" } # mine\n";
        assert_eq!(
            set(inline, &k("ai.providers.x"), &json!({"kind": "b"})).unwrap(),
            "[ai.providers]\nx = { kind = \"b\" } # mine\n"
        );
        let section = "[ui.bar]  # bar section comment\nwidth_px = 900  # px\nold = 1\n";
        assert_eq!(
            set(section, &k("ui.bar"), &json!({"width_px": 800})).unwrap(),
            "[ui.bar]  # bar section comment\nwidth_px = 800  # px\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn unset_keeps_the_sections_comments() {
        let text = "[ui]\n# My display settings.\n\n# The theme.\ntheme = \"dark\"\nx = 1\n";
        assert_eq!(
            unset(text, &k("ui.theme")).unwrap(),
            "[ui]\n# My display settings.\n\nx = 1\n"
        );
        // The last key: the comments go before the next section.
        let text = "[ui]\n# My display settings.\n\ntheme = \"dark\"\n[log]\nlevel = \"info\"\n";
        assert_eq!(
            unset(text, &k("ui.theme")).unwrap(),
            "[ui]\n# My display settings.\n\n[log]\nlevel = \"info\"\n"
        );
        // Or at the end of the file.
        let text = "[ui]\n# My display settings.\n\ntheme = \"dark\"\n";
        assert_eq!(
            unset(text, &k("ui.theme")).unwrap(),
            "[ui]\n# My display settings.\n\n"
        );
        assert_eq!(
            unset("x = { a = 1, b = 2 }\n", &k("x.b")).unwrap(),
            "x = { a = 1 }\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn very_large_or_small_floats_use_an_exponent() {
        assert_eq!(set("", &k("x"), &json!(1e300)).unwrap(), "x = 1e300\n");
        assert_eq!(set("", &k("x"), &json!(2.5e-9)).unwrap(), "x = 2.5e-9\n");
        assert_eq!(set("", &k("x"), &json!(1.5)).unwrap(), "x = 1.5\n");
    }
}
