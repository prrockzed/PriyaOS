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
    // A file of only comments: they are its "end" to `toml_edit`, so they
    // would land below the first key; they go back on top afterwards.
    let file_comments = document
        .as_table()
        .is_empty()
        .then(|| document.trailing().as_str().unwrap_or("").to_owned())
        .filter(|trailing| !trailing.trim().is_empty());
    let mut positions = Positions::after(&document);
    let mut slot = Slot::Table(document.as_table_mut());
    let mut header = None;
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
        header = slot.header_position().or(header);
    }
    for (section, comments) in slot.put(last, value, header, &mut positions)? {
        give_to_next_section(&mut document, section, &comments);
    }
    if let Some(comments) = file_comments {
        document.set_trailing("");
        put_on_top(&mut document, &comments);
    }
    Ok(style.restore(document.to_string()))
}

/// `text` without `key`. A key that isn't there is fine: nothing changes.
pub fn unset(text: &str, key: &KeyPath) -> Result<String, EditError> {
    let (last, parents) = split(key)?;
    let style = Style::of(text);
    let mut document = parse(text)?;
    let mut slot = Slot::Table(document.as_table_mut());
    // The section the key is written under: dotted and implicit tables
    // have no header of their own.
    let mut header = None;
    // How many parts lead to the table whose lines the key sits among: a
    // dotted group (`bar.a = 1`) is lines of the table above it.
    let mut lines_of = 0;
    for (i, part) in parents.iter().enumerate() {
        match slot.existing_child(part) {
            Some(child) => slot = child,
            None => return Ok(text.to_owned()),
        }
        header = slot.header_position().or(header);
        if !slot.is_dotted() {
            lines_of = i + 1;
        }
    }
    if let Some(comments) = slot.remove(last) {
        // Left over inside a dotted group: they go above the next line of
        // the table holding the group, else before the next section.
        let placed = lines_of < parents.len()
            && give_to_next_line(
                &mut document,
                &parents[..lines_of],
                &parents[lines_of],
                &comments,
            );
        if !placed {
            give_to_next_section(&mut document, header, &comments);
        }
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
    #[error("not changed: {}", summaries(problems, file))]
    Refused {
        file: std::path::PathBuf,
        problems: Vec<SettingsError>,
    },
}

impl UserFacing for ChangeError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        match self {
            Self::Edit(e) => e.user_message(),
            Self::Unreadable(e) => e.user_message(),
            Self::Refused { .. } => capitalised(&self.to_string()),
        }
    }
}

fn summaries(errors: &[SettingsError], file: &Path) -> String {
    errors
        .iter()
        .map(|e| e.summary(file))
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
    let found = sources
        .iter()
        .find(|s| s.file.as_deref() == Some(target.file));
    debug_assert!(
        found.is_none_or(|s| s.layer == target.layer),
        "the target's layer matches its source's"
    );
    let current = found
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
        // Only what applies before the target: lower layers, and earlier
        // files of its own layer.
        .filter(|s| {
            s.layer < target.layer
                || (s.layer == target.layer && s.file.as_deref() < Some(target.file))
        })
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
        Err(ChangeError::Refused {
            file: target.file.to_path_buf(),
            problems: added,
        })
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

    fn is_dotted(&self) -> bool {
        matches!(self, Self::Table(table) if table.is_dotted())
    }

    /// Where this table's `[header]` is, if it has one written out.
    fn header_position(&self) -> Option<isize> {
        match self {
            Self::Table(table) => header_of(table),
            Self::Inline(_) => None,
        }
    }

    /// Sets `key` here, in place where possible. `header` is the position
    /// of the section this table is written in. Returns section comments
    /// that lost their place, each with the position of the section they
    /// belong to, for the caller to put before the section after it.
    fn put(
        self,
        key: &str,
        value: &Value,
        header: Option<isize>,
        positions: &mut Positions,
    ) -> Result<Homeless, EditError> {
        let mut homeless = Homeless::new();
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
                    let own_header = header_of(old).or(header);
                    let stale: Vec<String> = old
                        .iter()
                        .map(|(k, _)| k.to_owned())
                        .filter(|k| !map.contains_key(k))
                        .collect();
                    let mut ours = String::new();
                    for gone in stale {
                        ours.extend(remove_from_table(old, &gone));
                    }
                    for (inner_key, inner) in map {
                        // A sub-section's comments stay its own.
                        homeless.extend(
                            Slot::Table(&mut *old).put(inner_key, inner, own_header, positions)?,
                        );
                    }
                    // This section's comments go before its first key, if
                    // it still has one, else before the section after it.
                    if !ours.is_empty() {
                        let first = old
                            .iter()
                            .find(|(_, item)| holds_key(item))
                            .map(|(k, _)| k.to_owned());
                        match first {
                            Some(first) => prepend_to_line(old, &first, &ours),
                            None => homeless.push((own_header, ours)),
                        }
                    }
                }
                (_, Value::Object(map)) => {
                    table.insert(key, Item::Table(new_table(map, positions)?));
                }
                (_, _) => {
                    // A table that held only tables gets its own header now;
                    // like any new section, it goes at the end.
                    if table.is_implicit() && !table.is_dotted() {
                        table.set_implicit(false);
                        table.set_position(Some(positions.next()));
                    }
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
        Ok(homeless)
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

/// Comments that lost their place, with the position of their section.
type Homeless = Vec<(Option<isize>, String)>;

/// The position of a table's own `[header]`, if it has one written out.
fn header_of(table: &Table) -> Option<isize> {
    if table.is_dotted() || table.is_implicit() {
        None
    } else {
        table.position()
    }
}

/// Whether `item` is written as a key line in its table (a value, or a
/// dotted key), so comments can sit above it.
fn holds_key(item: &Item) -> bool {
    item.is_value() || item.as_table().is_some_and(Table::is_dotted)
}

/// Removes `key` from a section with the comments written right above it.
/// Comments before a blank line belong to the section: they move to the
/// next key, or are returned when there is none.
fn remove_from_table(table: &mut Table, key: &str) -> Option<String> {
    let prefix = line_prefix(table, key);
    // The next key holding a value; a sub-table's comments go before its
    // own header, not here.
    let next = table
        .iter()
        .skip_while(|(k, _)| *k != key)
        .skip(1)
        .find(|(_, item)| holds_key(item))
        .map(|(k, _)| k.to_owned());
    table.remove(key)?;
    let kept = section_comments(&prefix);
    if kept.is_empty() {
        return None;
    }
    match next {
        Some(next) => {
            prepend_to_line(table, &next, kept);
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

/// Puts `comments` above the first line of the file: its first top-level
/// key, else its first section.
fn put_on_top(document: &mut DocumentMut, comments: &str) {
    let first = document
        .as_table()
        .iter()
        .find(|(_, item)| holds_key(item))
        .map(|(k, _)| k.to_owned());
    match first {
        Some(first) => prepend_to_line(document.as_table_mut(), &first, comments),
        None => give_to_next_section(document, None, comments),
    }
}

/// Puts `comments` above the first key line after `group` in the table at
/// `path`; false if there is none.
fn give_to_next_line(
    document: &mut DocumentMut,
    path: &[String],
    group: &str,
    comments: &str,
) -> bool {
    let mut slot = Slot::Table(document.as_table_mut());
    for part in path {
        match slot.existing_child(part) {
            Some(child) => slot = child,
            None => return false,
        }
    }
    let Slot::Table(table) = slot else {
        return false;
    };
    let next = table
        .iter()
        .skip_while(|(k, _)| *k != group)
        .skip(1)
        .find(|(_, item)| holds_key(item))
        .map(|(k, _)| k.to_owned());
    match next {
        Some(next) => {
            prepend_to_line(table, &next, comments);
            true
        }
        None => false,
    }
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
        Some(section) => prepend(section.decor_mut(), comments),
        None => {
            let old = document.trailing().as_str().unwrap_or("").to_owned();
            document.set_trailing(joined(comments, &old));
        }
    }
}

/// The comments above the line of `key` in `table`; see [`prepend_to_line`].
fn line_prefix(table: &Table, key: &str) -> String {
    match table.get(key).and_then(Item::as_table) {
        Some(inner) if inner.is_dotted() => inner
            .iter()
            .find(|(_, item)| holds_key(item))
            .map(|(first, _)| line_prefix(inner, first))
            .unwrap_or_default(),
        _ => table
            .key(key)
            .and_then(|k| k.leaf_decor().prefix())
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_owned(),
    }
}

/// Puts `comments` above the line of `key` in `table`. A dotted key
/// (`bar.width_px = 1`) keeps its line's comments on its innermost key.
fn prepend_to_line(table: &mut Table, key: &str, comments: &str) {
    let dotted = table
        .get(key)
        .and_then(Item::as_table)
        .is_some_and(Table::is_dotted);
    if dotted {
        if let Some(Item::Table(inner)) = table.get_mut(key) {
            let first = inner
                .iter()
                .find(|(_, item)| holds_key(item))
                .map(|(k, _)| k.to_owned());
            if let Some(first) = first {
                prepend_to_line(inner, &first, comments);
            }
        }
    } else if let Some(mut line) = table.key_mut(key) {
        prepend(line.leaf_decor_mut(), comments);
    }
}

/// Puts `comments` before what `decor` already has in front.
fn prepend(decor: &mut toml_edit::Decor, comments: &str) {
    let old = decor
        .prefix()
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .to_owned();
    decor.set_prefix(joined(comments, &old));
}

/// `first` then `then`, without doubling the blank line between them.
fn joined(first: &str, then: &str) -> String {
    let ends_blank = first.ends_with("\n\n") || first.ends_with("\n\r\n");
    let then = if ends_blank {
        then.trim_start_matches(['\r', '\n'])
    } else {
        then
    };
    format!("{first}{then}")
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
        // A line break is written escaped (`"a\nb"`), never as a multi-line
        // string, so the file's own line endings can't change the value.
        Value::String(s) if s.contains(['\n', '\r']) => serde_json::to_string(s)
            .ok()
            .and_then(|quoted| quoted.parse().ok())
            .unwrap_or_else(|| s.as_str().into()),
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

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_later_file_of_the_same_layer_cant_hide_a_bad_value() {
        let sources = vec![
            Source::toml(Layer::User, USER, ""),
            Source::toml(
                Layer::User,
                "/home/p/.config/priya/z.toml",
                "[ui]\ntheme = \"light\"\n",
            ),
        ];
        assert!(
            change(
                &sources,
                user(),
                Change::Set(&k("ui.theme"), &json!("darkk")),
                home()
            )
            .is_err()
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_problem_in_another_file_says_where_it_is() {
        let sources = vec![
            Source::toml(
                Layer::System,
                "/etc/priya/settings.toml",
                "[ui]\ntheme = \"darkk\"\n",
            ),
            Source::toml(Layer::User, USER, "[ui]\ntheme = \"dark\"\n"),
        ];
        let err = change(&sources, user(), Change::Unset(&k("ui.theme")), home()).unwrap_err();
        assert_eq!(
            err.user_message(),
            "Not changed: `ui.theme` must be light or dark (found 'darkk' in /etc/priya/settings.toml line 2)"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn unsetting_a_dotted_key_keeps_its_sections_comments_in_place() {
        let text = "[ai]\n# sec\n\nbar.width = 1\n\n[z]\na = 1\n";
        assert_eq!(
            unset(text, &k("ai.bar.width")).unwrap(),
            "[ai]\n# sec\n\n[z]\na = 1\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn text_with_line_breaks_survives_any_line_endings() {
        let crlf = "[ui]\r\ntheme = \"dark\"\r\n";
        let after = set(crlf, &k("ui.note"), &json!("a\nb")).unwrap();
        assert_eq!(after, "[ui]\r\ntheme = \"dark\"\r\nnote = \"a\\nb\"\r\n");
        let value: toml_edit::DocumentMut = after.parse().unwrap();
        assert_eq!(value["ui"]["note"].as_str(), Some("a\nb"));
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_section_given_new_keys_keeps_its_comments() {
        let text = "[ui]\n# sec\n\ntheme = \"dark\"\n[log]\nlevel = \"info\"\n";
        assert_eq!(
            set(text, &k("ui"), &json!({"reduced_motion": true})).unwrap(),
            "[ui]\n# sec\n\nreduced_motion = true\n[log]\nlevel = \"info\"\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_first_value_in_an_implicit_table_gets_a_header_at_the_end() {
        let text = "# c\n[ui.bar]\nwidth_px = 900\n";
        assert_eq!(
            set(text, &k("ui.theme"), &json!("light")).unwrap(),
            "# c\n[ui.bar]\nwidth_px = 900\n\n[ui]\ntheme = \"light\"\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn moved_comments_dont_double_blank_lines() {
        let text = "# file comment\n\nsettings_version = 1\n\n[ui]\ntheme = \"dark\"\n";
        assert_eq!(
            unset(text, &k("settings_version")).unwrap(),
            "# file comment\n\n[ui]\ntheme = \"dark\"\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn an_emptied_section_keeps_its_comments_before_the_next_one() {
        let text = "[ai]\nx = 1\n\n[ui]\n# sec\n\ntheme = \"dark\"\n\n[log]\nlevel = \"info\"\n";
        assert_eq!(
            set(text, &k("ui"), &json!({})).unwrap(),
            "[ai]\nx = 1\n\n[ui]\n# sec\n\n[log]\nlevel = \"info\"\n"
        );
        // A sub-section's comments stay with it, not its parent.
        let text =
            "[ui]\ntheme = \"dark\"\n\n[ui.bar]\n# barsec\n\nw = 1\n\n[log]\nlevel = \"info\"\n";
        assert_eq!(
            set(text, &k("ui"), &json!({"theme": "light", "bar": {}})).unwrap(),
            "[ui]\ntheme = \"light\"\n\n[ui.bar]\n# barsec\n\n[log]\nlevel = \"info\"\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_dotted_key_takes_the_comments_above_a_removed_one() {
        let text = "[ui]\n# section\n\ntheme = \"dark\"\nbar.width_px = 900\n";
        assert_eq!(
            unset(text, &k("ui.theme")).unwrap(),
            "[ui]\n# section\n\nbar.width_px = 900\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn a_comments_only_file_keeps_them_on_top() {
        assert_eq!(set("# hi\n", &k("x"), &json!(1)).unwrap(), "# hi\nx = 1\n");
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn only_a_comments_only_file_moves_its_comments_to_the_top() {
        assert_eq!(
            set("x = 1\n# end note\n", &k("x"), &json!(2)).unwrap(),
            "x = 2\n# end note\n"
        );
        let text = "[ui]\ntheme = \"dark\"\n# end note\n";
        let after = set(text, &k("settings_version"), &json!(1)).unwrap();
        assert!(after.ends_with("# end note\n"), "{after}");
        assert_eq!(
            set("# hi\n", &k("ui.theme"), &json!("dark")).unwrap(),
            "# hi\n[ui]\ntheme = \"dark\"\n"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn removing_a_dotted_group_keeps_the_sections_comments() {
        assert_eq!(
            unset("[ui]\n# sec\n\nbar.w = 1\nx = 1\n", &k("ui.bar")).unwrap(),
            "[ui]\n# sec\n\nx = 1\n"
        );
        // A comment about the removed line goes with it.
        assert_eq!(
            unset("[ui]\n# about bar\nbar.w = 1\nx = 1\n", &k("ui.bar")).unwrap(),
            "[ui]\nx = 1\n"
        );
        // The last key of a group: the comments go above the table's next line.
        assert_eq!(
            unset(
                "[ui]\nbar.a = 1\n# sec\n\nbar.b = 2\nx = 3\n",
                &k("ui.bar.b")
            )
            .unwrap(),
            "[ui]\nbar.a = 1\n# sec\n\nx = 3\n"
        );
    }
}
