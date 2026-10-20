//! Settings errors, each with the file, line and key it is about, and a plain
//! message for the owner.

use std::fmt;
use std::path::{Path, PathBuf};

use priya_api::error::{ErrorKind, UserFacing};

/// One thing wrong with the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsError {
    /// The file it is in; none for a request's overrides.
    pub file: Option<PathBuf>,
    /// How to show `file`: under the home folder it starts with `~`.
    pub shown_file: Option<String>,
    pub line: Option<u32>,
    /// The dotted key, such as `ui.theme`, when the problem has one.
    pub key: Option<String>,
    pub problem: Problem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The file isn't valid TOML.
    Syntax(String),
    /// No setting has this key.
    UnknownKey { suggestion: Option<String> },
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
    /// Anything else, already in plain words.
    Other(String),
}

impl SettingsError {
    /// Where it is, for messages: `~/.config/priya/settings.toml line 3`.
    fn place(&self) -> String {
        match (&self.shown_file, self.line) {
            (Some(file), Some(line)) => format!("{file} line {line}"),
            (Some(file), None) => file.clone(),
            (None, _) => "the settings given with this request".into(),
        }
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self.key.as_deref().unwrap_or("");
        let place = self.place();
        match &self.problem {
            Problem::Syntax(message) => write!(f, "{place} isn't valid TOML: {message}"),
            Problem::UnknownKey {
                suggestion: Some(other),
            } => write!(
                f,
                "`{key}` isn't a setting; did you mean `{other}`? It is in {place}"
            ),
            Problem::UnknownKey { suggestion: None } => {
                write!(f, "`{key}` isn't a setting; it is in {place}")
            }
            Problem::Invalid { expected, found } => {
                write!(f, "`{key}` must be {expected}; {}", wrote(found, &place))
            }
            Problem::Rejected { message, found } => {
                write!(f, "`{key}`: {message}; {}", wrote(found, &place))
            }
            Problem::Missing { field } => {
                write!(f, "`{key}` needs a `{field}`; add it in {place}")
            }
            Problem::Other(message) => write!(f, "`{key}`: {message}, in {place}"),
        }
    }
}

impl std::error::Error for SettingsError {}

impl UserFacing for SettingsError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        self.to_string()
    }
}

/// Everything wrong with one attempt to load the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsErrors(pub Vec<SettingsError>);

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
        self.to_string()
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
