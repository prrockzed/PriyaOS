//! [`KeyPath`]: a settings key as its parts, such as `ui` › `bar` ›
//! `width_px`.
//!
//! Keys are kept as parts, never as one dotted string, because a TOML key
//! may itself contain a dot when quoted (`[ai.providers."my.lab"]`). They
//! print the TOML way, quoting such parts, and parse back the same way.

use std::fmt;

use serde::{Serialize, Serializer};

/// A settings key. Ordered so that a key's descendants come right after it.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyPath(Vec<String>);

impl KeyPath {
    /// Parses TOML key syntax: `ui.theme`, `ai.providers."my.lab".kind`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let refuse = || format!("`{text}` isn't a valid settings key; write one like ui.theme.");
        let keys = toml_edit::Key::parse(text).map_err(|_| refuse())?;
        if keys.is_empty() {
            return Err(refuse());
        }
        Ok(Self(keys.iter().map(|k| k.get().to_owned()).collect()))
    }

    pub fn from_parts<S: Into<String>>(parts: impl IntoIterator<Item = S>) -> Self {
        Self(parts.into_iter().map(Into::into).collect())
    }

    pub fn parts(&self) -> &[String] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// This key with `part` added.
    pub fn child(&self, part: &str) -> Self {
        let mut parts = self.0.clone();
        parts.push(part.to_owned());
        Self(parts)
    }

    /// The key holding this one; none for a top-level key.
    pub fn parent(&self) -> Option<Self> {
        let (_, parents) = self.0.split_last()?;
        (!parents.is_empty()).then(|| Self(parents.to_vec()))
    }

    pub fn last(&self) -> Option<&str> {
        self.0.last().map(String::as_str)
    }

    /// Whether `self` is `other` or below it.
    pub fn starts_with(&self, other: &Self) -> bool {
        self.0.starts_with(&other.0)
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, part) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            let bare = !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            if bare {
                f.write_str(part)?;
            } else {
                write!(f, "{}", toml_edit::Key::new(part.as_str()).display_repr())?;
            }
        }
        Ok(())
    }
}

impl Serialize for KeyPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-03
    // suite: unit
    #[test]
    fn keys_with_dots_in_quotes_stay_whole() {
        let key = KeyPath::parse(r#"ai.providers."my.lab".kind"#).unwrap();
        assert_eq!(key.parts(), ["ai", "providers", "my.lab", "kind"]);
        assert_eq!(key.to_string(), r#"ai.providers."my.lab".kind"#);
        assert_eq!(KeyPath::parse(&key.to_string()).unwrap(), key);
        assert_eq!(KeyPath::parse("ui.bar").unwrap().to_string(), "ui.bar");
        for bad in ["", "ui..x", "ui.", "a b"] {
            assert!(KeyPath::parse(bad).is_err(), "{bad:?}");
        }

        // Descendants sort right after their key, before its siblings.
        let mut keys: Vec<KeyPath> = ["ui.bar.x", "ui0", "ui", "ui.bar", "ui.a"]
            .iter()
            .map(|k| KeyPath::parse(k).unwrap())
            .collect();
        keys.sort();
        let shown: Vec<String> = keys.iter().map(ToString::to_string).collect();
        assert_eq!(shown, ["ui", "ui.a", "ui.bar", "ui.bar.x", "ui0"]);
        assert_eq!(
            KeyPath::parse("ui.bar.x").unwrap().parent(),
            Some(KeyPath::parse("ui.bar").unwrap())
        );
        assert_eq!(KeyPath::parse("ui").unwrap().parent(), None);
    }
}
