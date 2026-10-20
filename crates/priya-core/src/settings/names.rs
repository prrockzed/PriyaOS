//! Names chosen by the owner in settings, checked once where they are read.

use std::borrow::{Borrow, Cow};
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

/// The name of an `ai.providers` entry, as used in `fixed:<name>/<model>`:
/// lowercase letters, digits, `-` and `_`, starting with a letter or digit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProviderName(String);

impl ProviderName {
    pub const PATTERN: &'static str = "^[a-z0-9][a-z0-9_-]*$";

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn is_valid(name: &str) -> bool {
        let mut chars = name.chars();
        chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    }
}

impl TryFrom<String> for ProviderName {
    type Error = String;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        if Self::is_valid(&name) {
            Ok(Self(name))
        } else {
            Err(format!(
                "'{name}' can't be a provider name: use lowercase letters, digits, - and _"
            ))
        }
    }
}

impl From<ProviderName> for String {
    fn from(name: ProviderName) -> Self {
        name.0
    }
}

impl Borrow<str> for ProviderName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl JsonSchema for ProviderName {
    fn schema_name() -> Cow<'static, str> {
        "ProviderName".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "pattern": Self::PATTERN })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn provider_names_are_lowercase_words() {
        for good in ["anthropic", "lm-studio", "ollama_2", "0x"] {
            assert!(ProviderName::try_from(good.to_owned()).is_ok(), "{good}");
        }
        for bad in ["", "Anthropic", "-x", "_x", "a b", "a/b", "a.b"] {
            let err = ProviderName::try_from(bad.to_owned()).unwrap_err();
            assert!(err.contains("lowercase letters"), "{bad}: {err}");
        }
    }
}
