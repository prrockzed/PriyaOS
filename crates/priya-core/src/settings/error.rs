//! Settings errors, each with the place and key it is about, and a plain
//! message for the owner.

use std::fmt;
use std::path::Path;

use priya_api::error::{ErrorKind, UserFacing};

use super::key::KeyPath;
use super::layers::{Layer, Origin};

/// One thing wrong with the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsError {
    /// Where it is: layer, file and line, when known.
    pub origin: Option<Origin>,
    /// The file as shown to the owner: under the home folder it starts with
    /// `~`.
    pub shown_file: Option<String>,
    /// The key it is about, when it has one.
    pub key: Option<KeyPath>,
    pub problem: Problem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The file isn't valid TOML. `details` is the parser's own wording.
    Syntax { message: String, details: String },
    /// No setting has this key.
    UnknownKey { suggestion: Option<KeyPath> },
    /// The value has the wrong form. `found` is the value as written, when
    /// it is short enough to repeat.
    Invalid {
        expected: String,
        found: Option<String>,
    },
    /// The value has the right form but was refused; `message` says why.
    Rejected {
        message: String,
        found: Option<String>,
    },
    /// A table lacks a key it needs.
    Missing { field: String },
    /// The file is for a newer version of PriyaOS.
    TooNew { found: u32, supported: u32 },
    /// Anything else, already in plain words.
    Other(String),
}

impl SettingsError {
    pub(crate) fn new(
        origin: Option<Origin>,
        key: Option<KeyPath>,
        problem: Problem,
        home: Option<&Path>,
    ) -> Self {
        let shown_file = origin
            .as_ref()
            .and_then(|o| o.file.as_deref())
            .map(|file| shown(file, home));
        Self {
            origin,
            shown_file,
            key,
            problem,
        }
    }

    /// Where it is: `~/.config/priya/settings.toml line 3`.
    fn place(&self) -> String {
        let line = self.origin.as_ref().and_then(|o| o.line);
        match (&self.shown_file, line, &self.origin) {
            (Some(file), Some(line), _) => format!("{file} line {line}"),
            (Some(file), None, _) => file.clone(),
            (None, _, Some(origin)) if origin.layer == Layer::Task => {
                "the settings given with this request".into()
            }
            _ => "your settings".into(),
        }
    }

    /// The problem without where it is, for a change that was refused:
    /// "`ui.theme` must be light or dark (you gave 'darkk')".
    pub fn summary(&self) -> String {
        let key = match &self.key {
            Some(key) => format!("`{key}`"),
            None => String::new(),
        };
        let gave = |found: &Option<String>| match found {
            Some(value) => format!(" (you gave {value})"),
            None => String::new(),
        };
        match &self.problem {
            Problem::Syntax { message, .. } => format!("the file isn't valid TOML: {message}"),
            Problem::UnknownKey {
                suggestion: Some(other),
            } => format!("{key} isn't a setting; did you mean `{other}`?"),
            Problem::UnknownKey { suggestion: None } => format!("{key} isn't a setting"),
            Problem::Invalid { expected, found } => {
                format!("{key} must be {expected}{}", gave(found))
            }
            Problem::Rejected { message, found } => format!("{key}: {message}{}", gave(found)),
            Problem::Missing { field } => format!("{key} needs a `{field}`"),
            Problem::TooNew { .. } => "the file is for a newer version of PriyaOS".into(),
            Problem::Other(message) if self.key.is_some() => format!("{key}: {message}"),
            Problem::Other(message) => message.clone(),
        }
    }

    /// For sorting: the order the owner meets problems in their files.
    pub(crate) fn order(&self) -> impl Ord + '_ {
        let origin = self.origin.as_ref();
        (
            origin.map(|o| o.layer),
            origin.and_then(|o| o.file.as_deref()),
            origin.and_then(|o| o.line),
            &self.key,
        )
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let place = self.place();
        let key = match &self.key {
            Some(key) => format!("`{key}`"),
            None => String::new(),
        };
        match &self.problem {
            Problem::Syntax { message, .. } => write!(f, "{place} isn't valid TOML: {message}"),
            Problem::UnknownKey {
                suggestion: Some(other),
            } => write!(
                f,
                "{key} isn't a setting; did you mean `{other}`? It is in {place}"
            ),
            Problem::UnknownKey { suggestion: None } => {
                write!(f, "{key} isn't a setting; it is in {place}")
            }
            Problem::Invalid { expected, found } => {
                write!(f, "{key} must be {expected}; {}", wrote(found, &place))
            }
            Problem::Rejected { message, found } => {
                write!(f, "{key}: {message}; {}", wrote(found, &place))
            }
            Problem::Missing { field } => {
                write!(f, "{key} needs a `{field}`; add it in {place}")
            }
            Problem::TooNew { found, supported } => write!(
                f,
                "{place} is for a newer version of PriyaOS (settings format {found}; this one reads up to {supported}). Update PriyaOS, or remove the newer settings"
            ),
            Problem::Other(message) if self.key.is_some() => {
                write!(f, "{key}: {message}, in {place}")
            }
            Problem::Other(message) => write!(f, "{place}: {message}"),
        }
    }
}

impl std::error::Error for SettingsError {}

impl UserFacing for SettingsError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        capitalised(&self.to_string())
    }
}

/// Everything wrong with one attempt to load the settings, in the order the
/// owner meets it: by layer, file and line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsErrors(pub Vec<SettingsError>);

impl SettingsErrors {
    pub(crate) fn sorted(mut errors: Vec<SettingsError>) -> Self {
        errors.sort_by(|a, b| a.order().cmp(&b.order()));
        Self(errors)
    }
}

impl fmt::Display for SettingsErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, error) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for SettingsErrors {}

impl UserFacing for SettingsErrors {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        self.0
            .iter()
            .map(UserFacing::user_message)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// "you wrote 'darkk' in <place>", or just "see <place>".
fn wrote(found: &Option<String>, place: &str) -> String {
    match found {
        Some(value) => format!("you wrote {value} in {place}"),
        None => format!("see {place}"),
    }
}

/// `path` with the home folder written as `~`.
pub(crate) fn shown(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

pub(crate) fn capitalised(message: &str) -> String {
    let mut chars = message.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
