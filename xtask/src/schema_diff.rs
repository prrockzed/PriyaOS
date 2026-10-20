//! Classifies the difference between two versions of a JSON Schema as
//! additive or breaking (09-testing §6, sub-phase 0.9).
//!
//! What breaks depends on which way the data flows:
//! - **Input** (method params, settings, manifests): what used to be
//!   accepted must still be. A new required property, a removed property
//!   or enum variant, a type that no longer fits, a tightened constraint
//!   all break; a new optional property, a new variant, a loosened
//!   constraint don't.
//! - **Output** (method results, notifications): what readers rely on must
//!   still be there. A removed property, one that may now be missing, a
//!   new type or a loosened constraint break; a new property, a new enum
//!   variant or a tightened constraint don't.
//! - **Both** (event payloads, read back years later): only new optional
//!   properties and new enum variants are additive.
//!
//! Annotations (`title`, `description`, `default`, `examples`…) never
//! count.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

/// The properties of a schema that has none.
static EMPTY: std::sync::LazyLock<serde_json::Map<String, Value>> =
    std::sync::LazyLock::new(serde_json::Map::new);

/// Which way the data described by a schema flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Input,
    Output,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    Additive,
    Breaking,
}

/// One difference: where (a JSON-pointer-like path), how bad, and what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub class: Class,
    pub what: String,
}

/// The differences between `old` and `new`, two versions of one schema.
pub fn diff(old: &Value, new: &Value, position: Position) -> Vec<Change> {
    diff_part(old, new, "", position)
}

/// The differences in the part of two documents at `pointer` (such as
/// `/$defs/params`), whose `$ref`s point into the whole document. A part
/// only one side has counts as a schema that accepted anything.
pub fn diff_part(old: &Value, new: &Value, pointer: &str, position: Position) -> Vec<Change> {
    static ANYTHING: Value = Value::Bool(true);
    let mut walk = Walk {
        old_root: old,
        new_root: new,
        position,
        changes: Vec::new(),
        path_refs: Vec::new(),
    };
    walk.node(
        old.pointer(pointer).unwrap_or(&ANYTHING),
        new.pointer(pointer).unwrap_or(&ANYTHING),
        "",
    );
    walk.changes
}

/// Whether a change made constraints tighter or looser (in input terms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tighter {
    Yes,
    No,
}

struct Walk<'a> {
    old_root: &'a Value,
    new_root: &'a Value,
    position: Position,
    changes: Vec<Change>,
    /// `$ref` pairs being compared on the current path, so recursive
    /// schemas end.
    path_refs: Vec<(String, String)>,
}

impl<'a> Walk<'a> {
    fn push(&mut self, path: &str, class: Class, what: impl Into<String>) {
        self.changes.push(Change {
            path: if path.is_empty() {
                "/".into()
            } else {
                path.to_owned()
            },
            class,
            what: what.into(),
        });
    }

    /// A constraint change: tighter breaks input, looser breaks output.
    fn constraint(&mut self, path: &str, tighter: Tighter, what: String) {
        let class = match (self.position, tighter) {
            (Position::Both, _) => Class::Breaking,
            (Position::Input, Tighter::Yes) | (Position::Output, Tighter::No) => Class::Breaking,
            (Position::Input, Tighter::No) | (Position::Output, Tighter::Yes) => Class::Additive,
        };
        self.push(path, class, what);
    }

    /// Follows a local `$ref` (`#/$defs/Name`) in `root`.
    fn resolve(root: &'a Value, node: &'a Value) -> &'a Value {
        let mut node = node;
        for _ in 0..32 {
            let Some(reference) = node.get("$ref").and_then(Value::as_str) else {
                return node;
            };
            let Some(pointer) = reference.strip_prefix('#') else {
                return node;
            };
            match root.pointer(pointer) {
                Some(target) => node = target,
                None => return node,
            }
        }
        node
    }

    fn node(&mut self, old: &'a Value, new: &'a Value, path: &str) {
        let old_ref = old.get("$ref").and_then(Value::as_str).map(str::to_owned);
        let new_ref = new.get("$ref").and_then(Value::as_str).map(str::to_owned);
        let pair = old_ref.zip(new_ref);
        if let Some(pair) = &pair {
            if self.path_refs.contains(pair) {
                return; // a schema that contains itself: compared already
            }
            self.path_refs.push(pair.clone());
        }
        self.compare(
            Self::resolve(self.old_root, old),
            Self::resolve(self.new_root, new),
            path,
        );
        if pair.is_some() {
            self.path_refs.pop();
        }
    }

    fn compare(&mut self, old: &'a Value, new: &'a Value, path: &str) {
        match (old, new) {
            (Value::Bool(a), Value::Bool(b)) if a == b => return,
            (Value::Bool(_), _) | (_, Value::Bool(_)) => {
                let tighter = if old == &Value::Bool(true) || new == &Value::Bool(false) {
                    Tighter::Yes
                } else {
                    Tighter::No
                };
                self.constraint(path, tighter, "accepts something else entirely".into());
                return;
            }
            _ => {}
        }
        self.types(old, new, path);
        let values = (allowed_values(old), allowed_values(new));
        self.choices(&values, path);
        // Alternatives that are only fixed values were compared as values.
        if !matches!(values, (Some(_), Some(_))) {
            self.alternatives(old, new, "anyOf", path);
            self.alternatives(old, new, "oneOf", path);
        }
        self.properties(old, new, path);
        self.patterns(old, new, path);
        self.additional(old, new, path);
        match (old.get("items"), new.get("items")) {
            (Some(a), Some(b)) => self.node(a, b, &format!("{path}/items")),
            (None, Some(_)) => {
                self.constraint(path, Tighter::Yes, "its items now have a schema".into())
            }
            (Some(_), None) => self.constraint(
                path,
                Tighter::No,
                "its items no longer have a schema".into(),
            ),
            (None, None) => {}
        }
        self.bounds(old, new, path);
    }

    /// `type`, as a set (absent means anything). `number` covers `integer`.
    fn types(&mut self, old: &Value, new: &Value, path: &str) {
        let set = type_set;
        let covers = |set: &BTreeSet<String>, t: &str| {
            set.contains(t) || (t == "integer" && set.contains("number"))
        };
        match (set(old), set(new)) {
            (Some(a), Some(b)) if a == b => {}
            (Some(a), Some(b)) => {
                for gone in a.iter().filter(|t| !covers(&b, t)) {
                    self.constraint(
                        path,
                        Tighter::Yes,
                        format!("no longer allows type `{gone}`"),
                    );
                }
                for added in b.iter().filter(|t| !covers(&a, t)) {
                    self.constraint(path, Tighter::No, format!("now allows type `{added}`"));
                }
            }
            (None, Some(b)) => {
                self.constraint(path, Tighter::Yes, format!("now must be {}", join(&b)))
            }
            (Some(a), None) => {
                self.constraint(path, Tighter::No, format!("no longer must be {}", join(&a)))
            }
            (None, None) => {}
        }
    }

    /// Fixed values (`enum`, `const`, or alternatives that are all fixed
    /// values, as schemars writes an enum with documented variants).
    fn choices(
        &mut self,
        values: &(Option<BTreeSet<String>>, Option<BTreeSet<String>>),
        path: &str,
    ) {
        match values {
            (Some(a), Some(b)) if a == b => {}
            (Some(a), Some(b)) => {
                for gone in a.difference(b) {
                    // Readers and writers both may hold the old value.
                    self.push(path, Class::Breaking, format!("value {gone} was removed"));
                }
                for added in b.difference(a) {
                    self.push(path, Class::Additive, format!("value {added} was added"));
                }
            }
            (None, Some(_)) => {
                self.constraint(path, Tighter::Yes, "now limited to set values".into())
            }
            (Some(_), None) => {
                self.constraint(path, Tighter::No, "no longer limited to set values".into())
            }
            (None, None) => {}
        }
    }

    /// `anyOf`/`oneOf`: alternatives matched by what they are (a `$ref`, a
    /// tag, a `type`), not by position. A `$ref` whose type was renamed
    /// still pairs with its old self when it is the only one left on each
    /// side.
    fn alternatives(&mut self, old: &'a Value, new: &'a Value, keyword: &str, path: &str) {
        let (old_list, new_list) = match (alternatives(old, keyword), alternatives(new, keyword)) {
            (None, None) => return,
            (Some(a), Some(b)) => (a, b),
            (None, Some(_)) => {
                return self.constraint(
                    path,
                    Tighter::Yes,
                    format!("now has to match `{keyword}`"),
                );
            }
            (Some(_), None) => {
                return self.constraint(
                    path,
                    Tighter::No,
                    format!("no longer has to match `{keyword}`"),
                );
            }
        };
        let mut gone: Vec<&String> = old_list
            .keys()
            .filter(|k| !new_list.contains_key(*k))
            .collect();
        let mut added: Vec<&String> = new_list
            .keys()
            .filter(|k| !old_list.contains_key(*k))
            .collect();
        for (key, old_item) in &old_list {
            if let Some(new_item) = new_list.get(key) {
                self.node(old_item, new_item, &format!("{path}/{keyword}/{key}"));
            }
        }
        let refs = |keys: &[&String]| -> Vec<String> {
            keys.iter()
                .filter(|k| k.starts_with("ref:"))
                .map(|k| (*k).clone())
                .collect()
        };
        if let ([old_ref], [new_ref]) = (refs(&gone).as_slice(), refs(&added).as_slice()) {
            self.node(
                old_list[old_ref],
                new_list[new_ref],
                &format!("{path}/{keyword}/{new_ref}"),
            );
            gone.retain(|k| *k != old_ref);
            added.retain(|k| *k != new_ref);
        }
        // A tagged variant is like an enum value: removing one breaks
        // everyone, adding one breaks no one.
        for key in gone {
            if key.starts_with("tag:") {
                self.push(path, Class::Breaking, format!("variant {key} was removed"));
            } else {
                self.constraint(path, Tighter::Yes, format!("alternative {key} was removed"));
            }
        }
        for key in added {
            if key.starts_with("tag:") {
                self.push(path, Class::Additive, format!("variant {key} was added"));
            } else {
                self.constraint(path, Tighter::No, format!("alternative {key} was added"));
            }
        }
    }

    fn properties(&mut self, old: &'a Value, new: &'a Value, path: &str) {
        let props = |node: &'a Value| node.get("properties").and_then(Value::as_object);
        let required = |node: &Value| -> BTreeSet<String> {
            node.get("required")
                .and_then(Value::as_array)
                .map(|r| {
                    r.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        let (old_props, new_props) = (props(old).unwrap_or(&EMPTY), props(new).unwrap_or(&EMPTY));
        let (old_required, new_required) = (required(old), required(new));
        for (name, old_schema) in old_props {
            let here = format!("{path}/{name}");
            let Some(new_schema) = new_props.get(name) else {
                self.push(&here, Class::Breaking, format!("`{name}` was removed"));
                continue;
            };
            self.required_change(
                &here,
                name,
                old_required.contains(name),
                new_required.contains(name),
            );
            self.node(old_schema, new_schema, &here);
        }
        for name in new_props.keys().filter(|n| !old_props.contains_key(*n)) {
            let here = format!("{path}/{name}");
            if new_required.contains(name) && self.position != Position::Output {
                self.push(
                    &here,
                    Class::Breaking,
                    format!("`{name}` was added as required"),
                );
            } else {
                self.push(&here, Class::Additive, format!("`{name}` was added"));
            }
        }
        // Required names without a property schema on either side.
        let bare = |name: &String| !old_props.contains_key(name) && !new_props.contains_key(name);
        for name in old_required.union(&new_required).filter(|n| bare(n)) {
            self.required_change(
                &format!("{path}/{name}"),
                name,
                old_required.contains(name),
                new_required.contains(name),
            );
        }
    }

    fn required_change(&mut self, here: &str, name: &str, was: bool, is: bool) {
        match (was, is) {
            (false, true) => {
                let class = if self.position == Position::Output {
                    Class::Additive
                } else {
                    Class::Breaking
                };
                self.push(here, class, format!("`{name}` is now required"));
            }
            (true, false) => {
                let class = if self.position == Position::Input {
                    Class::Additive
                } else {
                    Class::Breaking
                };
                self.push(here, class, format!("`{name}` may now be missing"));
            }
            _ => {}
        }
    }

    /// `patternProperties`, matched by pattern. Any added or removed pattern
    /// changes which keys are checked how, so it counts as tighter.
    fn patterns(&mut self, old: &'a Value, new: &'a Value, path: &str) {
        let map = |node: &'a Value| node.get("patternProperties").and_then(Value::as_object);
        if map(old).is_none() && map(new).is_none() {
            return;
        }
        let (old_map, new_map) = (map(old).unwrap_or(&EMPTY), map(new).unwrap_or(&EMPTY));
        for (pattern, old_schema) in old_map {
            match new_map.get(pattern) {
                Some(new_schema) => self.node(old_schema, new_schema, &format!("{path}/{pattern}")),
                None => self.constraint(
                    path,
                    Tighter::Yes,
                    format!("keys matching `{pattern}` are no longer described"),
                ),
            }
        }
        for pattern in new_map.keys().filter(|p| !old_map.contains_key(*p)) {
            self.constraint(
                path,
                Tighter::Yes,
                format!("keys matching `{pattern}` now have a schema"),
            );
        }
    }

    /// `additionalProperties`: open (absent or `true`), closed (`false`), or
    /// a schema for the other values.
    fn additional(&mut self, old: &'a Value, new: &'a Value, path: &str) {
        #[derive(PartialEq)]
        enum Other<'v> {
            Open,
            Closed,
            Schema(&'v Value),
        }
        let other = |node: &'a Value| match node.get("additionalProperties") {
            None | Some(Value::Bool(true)) => Other::Open,
            Some(Value::Bool(false)) => Other::Closed,
            Some(schema) => Other::Schema(schema),
        };
        match (other(old), other(new)) {
            (Other::Schema(a), Other::Schema(b)) => {
                self.node(a, b, &format!("{path}/additionalProperties"));
            }
            (a, b) if a == b => {}
            (Other::Open, _) | (Other::Schema(_), Other::Closed) => {
                self.constraint(path, Tighter::Yes, "allows fewer other properties".into())
            }
            _ => self.constraint(path, Tighter::No, "allows more other properties".into()),
        }
    }

    /// Numbers, lengths, counts, patterns and string formats. (schemars
    /// writes integer widths as formats like `uint32`; those are left out.)
    fn bounds(&mut self, old: &Value, new: &Value, path: &str) {
        const LOWER: [&str; 5] = [
            "minimum",
            "exclusiveMinimum",
            "minLength",
            "minItems",
            "minProperties",
        ];
        const UPPER: [&str; 5] = [
            "maximum",
            "exclusiveMaximum",
            "maxLength",
            "maxItems",
            "maxProperties",
        ];
        let number = |node: &Value, key: &str| node.get(key).and_then(Value::as_f64);
        for (keys, lower) in [(LOWER, true), (UPPER, false)] {
            for key in keys {
                let tighter = match (number(old, key), number(new, key)) {
                    (Some(a), Some(b)) if (a - b).abs() < f64::EPSILON => continue,
                    (Some(a), Some(b)) => (b > a) == lower,
                    (None, Some(_)) => true,
                    (Some(_), None) => false,
                    (None, None) => continue,
                };
                let tighter = if tighter { Tighter::Yes } else { Tighter::No };
                self.constraint(path, tighter, format!("`{key}` changed"));
            }
        }
        fn format(node: &Value) -> Option<&Value> {
            node.get("format")
                .filter(|f| !f.as_str().is_some_and(is_number_format))
        }
        let pairs = [
            ("pattern", old.get("pattern"), new.get("pattern")),
            ("format", format(old), format(new)),
            (
                "uniqueItems",
                old.get("uniqueItems"),
                new.get("uniqueItems"),
            ),
        ];
        for (key, a, b) in pairs {
            match (a, b) {
                (Some(a), Some(b)) if a == b => {}
                (None, None) => {}
                (_, Some(_)) => {
                    self.constraint(path, Tighter::Yes, format!("`{key}` added or changed"))
                }
                (Some(_), None) => self.constraint(path, Tighter::No, format!("`{key}` removed")),
            }
        }
    }
}

/// A schema's `type` as a set; without one, the types of its
/// alternatives when each has one (`None`: anything).
fn type_set(node: &Value) -> Option<BTreeSet<String>> {
    match node.get("type") {
        Some(Value::String(one)) => Some([one.clone()].into()),
        Some(Value::Array(many)) => Some(
            many.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
        ),
        Some(_) => None,
        None => {
            let items = node
                .get("oneOf")
                .or_else(|| node.get("anyOf"))?
                .as_array()?;
            let mut types = BTreeSet::new();
            for item in items {
                types.extend(type_set(item)?);
            }
            Some(types)
        }
    }
}

/// schemars' integer and float widths (`uint32`, `int64`, `double`…).
fn is_number_format(format: &str) -> bool {
    matches!(format, "float" | "double")
        || format
            .strip_prefix("uint")
            .or_else(|| format.strip_prefix("int"))
            .is_some_and(|bits| bits.parse::<u8>().is_ok())
}

/// The values a schema is limited to, if it is: `enum`, `const`, or
/// `oneOf`/`anyOf` alternatives that are each one fixed value.
fn allowed_values(node: &Value) -> Option<BTreeSet<String>> {
    if let Some(Value::Array(values)) = node.get("enum") {
        return Some(values.iter().map(Value::to_string).collect());
    }
    if let Some(value) = node.get("const") {
        return Some([value.to_string()].into());
    }
    let items = node
        .get("oneOf")
        .or_else(|| node.get("anyOf"))?
        .as_array()?;
    let mut values = BTreeSet::new();
    for item in items {
        values.extend(allowed_values(item).filter(|v| v.len() == 1)?);
    }
    Some(values)
}

/// A list's alternatives by what each is; two alike get their position
/// added, so none is lost.
fn alternatives<'v>(node: &'v Value, keyword: &str) -> Option<BTreeMap<String, &'v Value>> {
    let items = node.get(keyword)?.as_array()?;
    let mut map = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        let key = alternative_key(item);
        let key = if map.contains_key(&key) {
            format!("{key}#{i}")
        } else {
            key
        };
        map.insert(key, item);
    }
    Some(map)
}

/// What an `anyOf`/`oneOf` alternative is: a `$ref`, an object tagged by a
/// fixed property (`{"kind": {"const": "a"}}`), or its `type`.
fn alternative_key(item: &Value) -> String {
    if let Some(reference) = item.get("$ref").and_then(Value::as_str) {
        return format!("ref:{reference}");
    }
    if let Some(props) = item.get("properties").and_then(Value::as_object) {
        for (name, schema) in props {
            if let Some(values) = allowed_values(schema).filter(|v| v.len() == 1) {
                let value = values.into_iter().next().unwrap_or_default();
                return format!("tag:{name}={value}");
            }
        }
    }
    match item.get("type") {
        Some(Value::String(one)) => format!("type:{one}"),
        Some(other) => format!("type:{other}"),
        None => "other".to_owned(),
    }
}

fn join(types: &BTreeSet<String>) -> String {
    types
        .iter()
        .map(|t| format!("`{t}`"))
        .collect::<Vec<_>>()
        .join(" or ")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// The worst class of the changes from `old` to `new`, if any.
    fn class(old: Value, new: Value, position: Position) -> Option<Class> {
        diff(&old, &new, position)
            .into_iter()
            .map(|c| c.class)
            .max()
    }

    fn object(properties: Value, required: &[&str]) -> Value {
        json!({"type": "object", "properties": properties, "required": required})
    }

    use Class::{Additive, Breaking};
    use Position::{Both, Input, Output};

    // covers: QA-08
    // suite: unit
    // acceptance: 0.9
    // regression: 0.9
    #[test]
    fn schema_diff_classifies_every_additive_and_breaking_rule() {
        let string = json!({"type": "string"});
        let one = object(json!({"a": string}), &["a"]);
        let cases: Vec<(&str, Value, Value, Position, Option<Class>)> = vec![
            ("nothing changed", one.clone(), one.clone(), Input, None),
            (
                "descriptions don't count",
                one.clone(),
                {
                    let mut v = one.clone();
                    v["description"] = json!("Now with words.");
                    v["properties"]["a"]["default"] = json!("x");
                    v
                },
                Input,
                None,
            ),
            // Additive.
            (
                "new optional input property",
                one.clone(),
                object(json!({"a": string, "b": string}), &["a"]),
                Input,
                Some(Additive),
            ),
            (
                "new optional event property",
                one.clone(),
                object(json!({"a": string, "b": string}), &["a"]),
                Both,
                Some(Additive),
            ),
            (
                "new output property, even required",
                one.clone(),
                object(json!({"a": string, "b": string}), &["a", "b"]),
                Output,
                Some(Additive),
            ),
            (
                "new enum variant in output",
                json!({"enum": ["a"]}),
                json!({"enum": ["a", "b"]}),
                Output,
                Some(Additive),
            ),
            (
                "new enum variant in input",
                json!({"enum": ["a"]}),
                json!({"enum": ["a", "b"]}),
                Input,
                Some(Additive),
            ),
            (
                "new const alternative",
                json!({"oneOf": [{"const": "a"}]}),
                json!({"oneOf": [{"const": "a"}, {"const": "b"}]}),
                Output,
                Some(Additive),
            ),
            (
                "loosened input maximum",
                json!({"type": "integer", "maximum": 5}),
                json!({"type": "integer", "maximum": 9}),
                Input,
                Some(Additive),
            ),
            (
                "loosened input pattern",
                json!({"type": "string", "pattern": "^a$"}),
                string.clone(),
                Input,
                Some(Additive),
            ),
            (
                "input property made optional",
                one.clone(),
                object(json!({"a": string}), &[]),
                Input,
                Some(Additive),
            ),
            (
                "input opened to more properties",
                json!({"type": "object", "additionalProperties": false}),
                json!({"type": "object"}),
                Input,
                Some(Additive),
            ),
            (
                "input accepts null too",
                string.clone(),
                json!({"type": ["string", "null"]}),
                Input,
                Some(Additive),
            ),
            (
                "tightened output constraint",
                json!({"type": "string"}),
                json!({"type": "string", "maxLength": 10}),
                Output,
                Some(Additive),
            ),
            // Breaking.
            (
                "removed property",
                one.clone(),
                object(json!({}), &[]),
                Output,
                Some(Breaking),
            ),
            (
                "renamed property",
                one.clone(),
                object(json!({"b": string}), &["b"]),
                Both,
                Some(Breaking),
            ),
            (
                "type change",
                one.clone(),
                object(json!({"a": {"type": "integer"}}), &["a"]),
                Input,
                Some(Breaking),
            ),
            (
                "new required input property",
                one.clone(),
                object(json!({"a": string, "b": string}), &["a", "b"]),
                Input,
                Some(Breaking),
            ),
            (
                "new required event property",
                one.clone(),
                object(json!({"a": string, "b": string}), &["a", "b"]),
                Both,
                Some(Breaking),
            ),
            (
                "removed enum variant",
                json!({"enum": ["a", "b"]}),
                json!({"enum": ["a"]}),
                Output,
                Some(Breaking),
            ),
            (
                "removed const alternative",
                json!({"oneOf": [{"const": "a"}, {"const": "b"}]}),
                json!({"oneOf": [{"const": "a"}]}),
                Input,
                Some(Breaking),
            ),
            (
                "tightened input maximum",
                json!({"type": "integer", "maximum": 9}),
                json!({"type": "integer", "maximum": 5}),
                Input,
                Some(Breaking),
            ),
            (
                "tightened input minimum",
                json!({"type": "integer"}),
                json!({"type": "integer", "minimum": 1}),
                Input,
                Some(Breaking),
            ),
            (
                "new input pattern",
                string.clone(),
                json!({"type": "string", "pattern": "^a$"}),
                Input,
                Some(Breaking),
            ),
            (
                "input closed to more properties",
                json!({"type": "object"}),
                json!({"type": "object", "additionalProperties": false}),
                Input,
                Some(Breaking),
            ),
            (
                "output may now be missing",
                one.clone(),
                object(json!({"a": string}), &[]),
                Output,
                Some(Breaking),
            ),
            (
                "output may now be null",
                string.clone(),
                json!({"type": ["string", "null"]}),
                Output,
                Some(Breaking),
            ),
            (
                "input property now required",
                object(json!({"a": string}), &[]),
                one.clone(),
                Input,
                Some(Breaking),
            ),
            (
                "event constraint tightened",
                json!({"type": "string"}),
                json!({"type": "string", "maxLength": 10}),
                Both,
                Some(Breaking),
            ),
            (
                "loosened output constraint",
                json!({"type": "string", "maxLength": 10}),
                string.clone(),
                Output,
                Some(Breaking),
            ),
        ];
        assert!(cases.len() >= 20);
        for (name, old, new, position, expected) in cases {
            assert_eq!(class(old, new, position), expected, "{name}");
        }
    }

    // covers: QA-08
    // suite: unit
    // acceptance: 0.9
    // regression: 0.9
    #[test]
    fn references_and_nesting_are_followed_and_reported_where_they_change() {
        let old = json!({
            "type": "object",
            "properties": {"event": {"$ref": "#/$defs/Event"}, "items": {"type": "array", "items": {"$ref": "#/$defs/Event"}}},
            "$defs": {"Event": {"type": "object", "properties": {"seq": {"type": "integer"}, "next": {"$ref": "#/$defs/Event"}}, "required": ["seq"]}}
        });
        let mut new = old.clone();
        new["$defs"]["Event"]["properties"]["seq"]["type"] = json!("string");
        let changes = diff(&old, &new, Output);
        let paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/event/seq",
                "/event/seq",
                "/items/items/seq",
                "/items/items/seq"
            ],
            "{changes:?}"
        );
        // In an output, a type that can no longer appear is fine; a new one
        // breaks readers.
        assert_eq!(changes.iter().filter(|c| c.class == Breaking).count(), 2);
        assert!(changes[0].what.contains("`integer`") || changes[1].what.contains("`integer`"));

        // Option<T> as anyOf [ref, null]: matched by kind, not position.
        let optional = json!({"anyOf": [{"$ref": "#/$defs/T"}, {"type": "null"}], "$defs": {"T": {"type": "string"}}});
        let swapped = json!({"anyOf": [{"type": "null"}, {"$ref": "#/$defs/T"}], "$defs": {"T": {"type": "string"}}});
        assert_eq!(diff(&optional, &swapped, Input), []);
        let dropped_null =
            json!({"anyOf": [{"$ref": "#/$defs/T"}], "$defs": {"T": {"type": "string"}}});
        assert_eq!(class(optional.clone(), dropped_null, Input), Some(Breaking));
        assert_eq!(class(json!(true), json!(false), Input), Some(Breaking));
        assert_eq!(
            class(json!({"type": "string"}), json!(true), Input),
            Some(Additive)
        );
        let map = |values: Value| json!({"type": "object", "additionalProperties": values});
        assert_eq!(
            class(
                map(json!({"type": "string"})),
                map(json!({"type": "integer"})),
                Input
            ),
            Some(Breaking)
        );
        assert_eq!(
            diff(&json!({"type": "string"}), &json!({"type": "string"}), Both),
            []
        );
        assert_eq!(diff(&one_root(), &one_root(), Both).len(), 0);
    }

    // covers: QA-08
    // suite: unit
    // acceptance: 0.9
    // regression: 0.9
    #[test]
    fn schema_diff_classifies_maps_tags_and_rewordings() {
        let provider = |props: Value, required: &[&str]| json!({"type": "object", "patternProperties": {"^[a-z]+$": object(props, required)}});
        let both = json!({"a": {"type": "string"}, "b": {"type": "string"}});
        let tagged = |tags: &[&str]| {
            let variants: Vec<Value> = tags
                .iter()
                .map(|t| {
                    object(
                        json!({"kind": {"const": t}, "x": {"type": "string"}}),
                        &["kind"],
                    )
                })
                .collect();
            json!({"oneOf": variants})
        };
        let documented = json!({"oneOf": [
            {"const": "a", "type": "string", "description": "A."},
            {"const": "b", "type": "string", "description": "B."}
        ]});
        let optional = |name: &str| {
            json!({"anyOf": [{"$ref": format!("#/$defs/{name}")}, {"type": "null"}],
                   "$defs": {name: {"type": "string"}}})
        };
        let cases: Vec<(&str, Value, Value, Position, Option<Class>)> = vec![
            (
                "a field removed under patternProperties",
                provider(both.clone(), &[]),
                provider(json!({"a": {"type": "string"}}), &[]),
                Input,
                Some(Breaking),
            ),
            (
                "a field made required under patternProperties",
                provider(both.clone(), &[]),
                provider(both.clone(), &["a"]),
                Input,
                Some(Breaking),
            ),
            (
                "a new key pattern",
                json!({"type": "object"}),
                json!({"type": "object", "patternProperties": {"^x": {"type": "string"}}}),
                Input,
                Some(Breaking),
            ),
            (
                "open map values given a schema",
                json!({"type": "object", "additionalProperties": true}),
                json!({"type": "object", "additionalProperties": {"type": "string"}}),
                Input,
                Some(Breaking),
            ),
            (
                "map values freed from their schema",
                json!({"type": "object", "additionalProperties": {"type": "string"}}),
                json!({"type": "object"}),
                Input,
                Some(Additive),
            ),
            (
                "array items given a schema",
                json!({"type": "array"}),
                json!({"type": "array", "items": {"type": "string"}}),
                Input,
                Some(Breaking),
            ),
            (
                "a bare name made required",
                json!({"type": "object"}),
                json!({"type": "object", "required": ["a"]}),
                Input,
                Some(Breaking),
            ),
            (
                "a tagged variant removed",
                tagged(&["a", "b"]),
                tagged(&["b"]),
                Both,
                Some(Breaking),
            ),
            (
                "a tagged variant added to an output",
                tagged(&["a"]),
                tagged(&["a", "b"]),
                Output,
                Some(Additive),
            ),
            (
                "an enum documented per variant",
                json!({"type": "string", "enum": ["a", "b"]}),
                documented.clone(),
                Both,
                None,
            ),
            (
                "a variant removed from a documented enum",
                documented,
                json!({"type": "string", "enum": ["a"]}),
                Output,
                Some(Breaking),
            ),
            (
                "the type inside an Option renamed",
                optional("A"),
                optional("B"),
                Input,
                None,
            ),
            (
                "integer widened to number in input",
                json!({"type": "integer"}),
                json!({"type": "number"}),
                Input,
                Some(Additive),
            ),
            (
                "an integer width changed",
                json!({"type": "integer", "format": "uint32", "minimum": 0}),
                json!({"type": "integer", "format": "uint64", "minimum": 0}),
                Both,
                None,
            ),
        ];
        for (name, old, new, position, expected) in cases {
            assert_eq!(class(old, new, position), expected, "{name}");
        }
        // Two alternatives of one kind both count.
        let twins = json!({"oneOf": [{"type": "object"}, {"type": "object", "title": "B"}]});
        assert_eq!(alternatives(&twins, "oneOf").expect("a list").len(), 2);
        assert!(is_number_format("int64") && !is_number_format("date-time"));
    }

    fn one_root() -> Value {
        json!({"$ref": "#/$defs/A", "$defs": {"A": {"$ref": "#/$defs/A"}}})
    }
}
