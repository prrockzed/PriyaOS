//! [`TempPriyaHome`]: a throwaway set of PriyaOS folders, so a test never
//! reads or writes the real `~/.config/priya` and friends.

use std::io;
use std::path::Path;
use std::process::Command;

use priya_core::paths::PriyaPaths;
use tempfile::TempDir;

/// Temporary config, data, runtime and state folders, deleted on drop.
#[derive(Debug)]
pub struct TempPriyaHome {
    // Kept for its Drop, which deletes the folders.
    _dir: TempDir,
    paths: PriyaPaths,
}

impl TempPriyaHome {
    pub fn new() -> io::Result<Self> {
        let dir = tempfile::Builder::new().prefix("priya-home-").tempdir()?;
        let paths = PriyaPaths {
            config: dir.path().join("config"),
            data: dir.path().join("data"),
            runtime: dir.path().join("runtime"),
            state: dir.path().join("state"),
        };
        for folder in [&paths.config, &paths.data, &paths.runtime, &paths.state] {
            std::fs::create_dir(folder)?;
        }
        Ok(Self { _dir: dir, paths })
    }

    pub fn paths(&self) -> &PriyaPaths {
        &self.paths
    }

    /// The `PRIYA_*_DIR` variables that point PriyaOS at these folders.
    pub fn env(&self) -> [(&'static str, &Path); 4] {
        [
            ("PRIYA_CONFIG_DIR", &self.paths.config),
            ("PRIYA_DATA_DIR", &self.paths.data),
            ("PRIYA_RUNTIME_DIR", &self.paths.runtime),
            ("PRIYA_STATE_DIR", &self.paths.state),
        ]
        .map(|(name, path)| (name, path.as_path()))
    }

    /// Exports [`env`](Self::env) to a child process (priyad, the CLI…).
    pub fn apply<'c>(&self, command: &'c mut Command) -> &'c mut Command {
        command.envs(self.env())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    // covers: QA-04
    // suite: unit
    #[test]
    fn temp_home_is_isolated_and_reaches_child_processes() {
        let home = TempPriyaHome::new().unwrap();
        let root = home.paths().data.parent().unwrap().to_path_buf();
        for (_, path) in home.env() {
            assert!(path.is_dir(), "{} exists", path.display());
            assert!(path.starts_with(&root));
        }

        // The same paths come back when PriyaOS resolves them from the
        // variables, as a child process would.
        let vars: Vec<(String, OsString)> = home
            .env()
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.as_os_str().to_owned()))
            .collect();
        let resolved = PriyaPaths::from_lookup(|name| {
            vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
        })
        .unwrap();
        assert_eq!(&resolved, home.paths());

        let mut command = Command::new("true");
        home.apply(&mut command);
        assert_eq!(command.get_envs().count(), 4);

        drop(home);
        assert!(!root.exists(), "the folders are removed on drop");
    }
}
