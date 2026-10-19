//! Where PriyaOS keeps its files, following the XDG Base Directory spec.
//!
//! Each folder can be overridden on its own (`PRIYA_CONFIG_DIR` and friends),
//! which tests and development rely on. macOS uses the same XDG fallbacks as
//! Linux, so paths look alike on both machines.

use std::ffi::OsString;
use std::path::PathBuf;

use priya_api::error::{ErrorKind, UserFacing};

const APP: &str = "priya";

/// The four folders PriyaOS uses for the current user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriyaPaths {
    /// Settings the user writes: `$XDG_CONFIG_HOME/priya`.
    pub config: PathBuf,
    /// Durable data such as the event store: `$XDG_DATA_HOME/priya`.
    pub data: PathBuf,
    /// Sockets and locks, gone after logout: `$XDG_RUNTIME_DIR/priya`.
    pub runtime: PathBuf,
    /// Logs and caches worth keeping across restarts: `$XDG_STATE_HOME/priya`.
    pub state: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathsError {
    #[error("HOME is not set or not absolute, and an XDG fallback needs it")]
    NoHome,
    #[error("{var} must be an absolute path, got {value:?}")]
    NotAbsolute { var: &'static str, value: PathBuf },
}

impl UserFacing for PathsError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        match self {
            Self::NoHome => "PriyaOS can't find your home folder.".into(),
            Self::NotAbsolute { var, .. } => {
                format!("The {var} setting must be a full path starting with /.")
            }
        }
    }
}

/// One folder's lookup order: an explicit override, then the XDG variable,
/// then the XDG fallback.
struct Rule {
    override_var: &'static str,
    xdg_var: &'static str,
    fallback: Fallback,
}

enum Fallback {
    /// A folder under `$HOME`.
    Home(&'static str),
    /// XDG gives no fallback for the runtime folder; use a per-user folder in
    /// the temp directory, as the spec suggests.
    Temp,
}

const CONFIG: Rule = Rule {
    override_var: "PRIYA_CONFIG_DIR",
    xdg_var: "XDG_CONFIG_HOME",
    fallback: Fallback::Home(".config"),
};
const DATA: Rule = Rule {
    override_var: "PRIYA_DATA_DIR",
    xdg_var: "XDG_DATA_HOME",
    fallback: Fallback::Home(".local/share"),
};
const RUNTIME: Rule = Rule {
    override_var: "PRIYA_RUNTIME_DIR",
    xdg_var: "XDG_RUNTIME_DIR",
    fallback: Fallback::Temp,
};
const STATE: Rule = Rule {
    override_var: "PRIYA_STATE_DIR",
    xdg_var: "XDG_STATE_HOME",
    fallback: Fallback::Home(".local/state"),
};

impl PriyaPaths {
    /// Resolves the folders from the process environment.
    pub fn from_env() -> Result<Self, PathsError> {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// Resolves the folders from any variable lookup. Tests pass a map here
    /// instead of changing the process environment, which is `unsafe` in
    /// edition 2024 and racy across threads.
    pub fn from_lookup(get: impl Fn(&str) -> Option<OsString>) -> Result<Self, PathsError> {
        let resolver = Resolver { get: &get };
        Ok(Self {
            config: resolver.resolve(&CONFIG)?,
            data: resolver.resolve(&DATA)?,
            runtime: resolver.resolve(&RUNTIME)?,
            state: resolver.resolve(&STATE)?,
        })
    }
}

struct Resolver<'a> {
    get: &'a dyn Fn(&str) -> Option<OsString>,
}

impl Resolver<'_> {
    fn var(&self, name: &str) -> Option<PathBuf> {
        (self.get)(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    }

    fn resolve(&self, rule: &Rule) -> Result<PathBuf, PathsError> {
        // An override names the PriyaOS folder itself. A relative one is a
        // mistake worth reporting, not guessing around.
        if let Some(path) = self.var(rule.override_var) {
            if !path.is_absolute() {
                return Err(PathsError::NotAbsolute {
                    var: rule.override_var,
                    value: path,
                });
            }
            return Ok(path);
        }
        // The XDG spec says relative values are invalid and must be ignored.
        if let Some(base) = self.var(rule.xdg_var).filter(|p| p.is_absolute()) {
            return Ok(base.join(APP));
        }
        match rule.fallback {
            Fallback::Home(sub) => {
                let home = self
                    .var("HOME")
                    .filter(|p| p.is_absolute())
                    .ok_or(PathsError::NoHome)?;
                Ok(home.join(sub).join(APP))
            }
            Fallback::Temp => {
                let tmp = self
                    .var("TMPDIR")
                    .filter(|p| p.is_absolute())
                    .unwrap_or_else(|| PathBuf::from("/tmp"));
                let folder = match self.var("USER") {
                    Some(user) => format!("{APP}-runtime-{}", user.display()),
                    None => format!("{APP}-runtime"),
                };
                Ok(tmp.join(folder))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn paths(vars: &[(&str, &str)]) -> Result<PriyaPaths, PathsError> {
        let map: HashMap<String, OsString> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
            .collect();
        PriyaPaths::from_lookup(|name| map.get(name).cloned())
    }

    // covers: infra
    // suite: unit
    // acceptance: 0.2
    // regression: 0.2
    #[test]
    fn paths_respect_xdg_overrides() {
        let p = paths(&[
            ("HOME", "/home/a"),
            ("XDG_CONFIG_HOME", "/xdg/config"),
            ("XDG_DATA_HOME", "/xdg/data"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
            ("XDG_STATE_HOME", "/xdg/state"),
            ("PRIYA_DATA_DIR", "/override/data"),
        ])
        .unwrap();
        assert_eq!(p.config, PathBuf::from("/xdg/config/priya"));
        assert_eq!(
            p.data,
            PathBuf::from("/override/data"),
            "override wins over XDG"
        );
        assert_eq!(p.runtime, PathBuf::from("/run/user/1000/priya"));
        assert_eq!(p.state, PathBuf::from("/xdg/state/priya"));
    }

    // covers: infra
    // suite: unit
    // acceptance: 0.2
    // regression: 0.2
    #[test]
    fn paths_fall_back_to_home_and_tmp() {
        let p = paths(&[("HOME", "/home/a"), ("USER", "a")]).unwrap();
        assert_eq!(p.config, PathBuf::from("/home/a/.config/priya"));
        assert_eq!(p.data, PathBuf::from("/home/a/.local/share/priya"));
        assert_eq!(p.state, PathBuf::from("/home/a/.local/state/priya"));
        assert_eq!(p.runtime, PathBuf::from("/tmp/priya-runtime-a"));

        let p = paths(&[("HOME", "/Users/a"), ("TMPDIR", "/var/folders/x/T")]).unwrap();
        assert_eq!(p.runtime, PathBuf::from("/var/folders/x/T/priya-runtime"));
    }

    // covers: infra
    // suite: unit
    // acceptance: 0.2
    // regression: 0.2
    #[test]
    fn relative_xdg_is_ignored_but_relative_override_is_an_error() {
        let p = paths(&[("HOME", "/home/a"), ("XDG_CONFIG_HOME", "rel/config")]).unwrap();
        assert_eq!(p.config, PathBuf::from("/home/a/.config/priya"));

        let err = paths(&[("HOME", "/home/a"), ("PRIYA_STATE_DIR", "state")]).unwrap_err();
        assert_eq!(
            err,
            PathsError::NotAbsolute {
                var: "PRIYA_STATE_DIR",
                value: PathBuf::from("state"),
            }
        );
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    // covers: infra
    // suite: unit
    // acceptance: 0.2
    // regression: 0.2
    #[test]
    fn missing_home_is_reported_only_when_needed() {
        assert_eq!(paths(&[]).unwrap_err(), PathsError::NoHome);

        let p = paths(&[
            ("XDG_CONFIG_HOME", "/c"),
            ("XDG_DATA_HOME", "/d"),
            ("XDG_STATE_HOME", "/s"),
            ("XDG_RUNTIME_DIR", "/r"),
        ])
        .unwrap();
        assert_eq!(p.config, PathBuf::from("/c/priya"));
    }
}
