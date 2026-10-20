//! Which settings changed between two versions, key by key, for one
//! `setting.changed` event each.

use serde_json::{Map, Value};

use super::key::KeyPath;
use super::model::Settings;

/// One setting that changed. `old` or `new` is empty when it was added or
/// removed.
#[derive(Debug, Clone, PartialEq)]
pub struct Changed {
    pub key: KeyPath,
    pub old: Option<Value>,
    pub new: Option<Value>,
}

/// Every value that differs, in key order. Lists are single values: a
/// changed list is one change.
pub fn diff(old: &Settings, new: &Settings) -> Vec<Changed> {
    let mut out = Vec::new();
    compare(
        Some(&as_json(old)),
        Some(&as_json(new)),
        &KeyPath::default(),
        &mut out,
    );
    out
}

fn as_json(settings: &Settings) -> Value {
    serde_json::to_value(settings).expect("settings always serialise to JSON")
}

fn compare(old: Option<&Value>, new: Option<&Value>, key: &KeyPath, out: &mut Vec<Changed>) {
    match (old, new) {
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            for name in keys(a, b) {
                compare(a.get(name), b.get(name), &key.child(name), out);
            }
        }
        // A table appeared or went: report each value in it.
        (Some(Value::Object(a)), None) => {
            for (name, value) in a {
                compare(Some(value), None, &key.child(name), out);
            }
        }
        (None, Some(Value::Object(b))) => {
            for (name, value) in b {
                compare(None, Some(value), &key.child(name), out);
            }
        }
        (a, b) if a != b => out.push(Changed {
            key: key.clone(),
            old: a.cloned(),
            new: b.cloned(),
        }),
        _ => {}
    }
}

/// The keys of both tables, sorted, each once.
fn keys<'a>(a: &'a Map<String, Value>, b: &'a Map<String, Value>) -> Vec<&'a String> {
    let mut names: Vec<&String> = a.keys().chain(b.keys()).collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::settings::{Provider, ProviderKind, ProviderName, Theme};

    // covers: CORE-03
    // suite: unit
    #[test]
    fn diff_lists_each_changed_value_once() {
        let old = Settings::default();
        assert!(diff(&old, &old).is_empty());

        let mut new = old.clone();
        new.ui.theme = Theme::Light;
        new.modules.enabled = vec!["weather".into()];
        new.ai.providers.insert(
            ProviderName::try_from("ollama".to_owned()).unwrap(),
            Provider {
                kind: ProviderKind::OpenaiCompatible,
                base_url: None,
                enabled: true,
            },
        );
        let changes: Vec<(String, Option<Value>, Option<Value>)> = diff(&old, &new)
            .into_iter()
            .map(|c| (c.key.to_string(), c.old, c.new))
            .collect();
        assert_eq!(
            changes,
            [
                (
                    "ai.providers.ollama.enabled".into(),
                    None,
                    Some(json!(true))
                ),
                (
                    "ai.providers.ollama.kind".into(),
                    None,
                    Some(json!("openai_compatible"))
                ),
                (
                    "modules.enabled".into(),
                    Some(json!([])),
                    Some(json!(["weather"]))
                ),
                ("ui.theme".into(), Some(json!("dark")), Some(json!("light"))),
            ]
        );
        // And back again, the other way round.
        let back: Vec<String> = diff(&new, &old)
            .into_iter()
            .map(|c| c.key.to_string())
            .collect();
        assert_eq!(back.len(), 4);
    }
}
