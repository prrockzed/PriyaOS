//! [`TempPriyaHome`]: a throwaway set of PriyaOS folders, so a test never
//! reads or writes the real `~/.config/priya` and friends. Child processes
//! also get a throwaway `HOME` and `XDG_*` folders, so nothing they start
//! touches the real home either.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use priya_core::paths::PriyaPaths;
use tempfile::TempDir;

/// Temporary PriyaOS folders and a temporary home, deleted on drop.
#[derive(Debug)]
pub struct TempPriyaHome {
    /// Deletes the folders when dropped.
    dir: TempDir,
    paths: PriyaPaths,
    /// `HOME` and the `XDG_*` base folders for child processes.
    home: PathBuf,
    xdg: [(&'static str, PathBuf); 5],
}

impl TempPriyaHome {
    /// Creates the folders. The runtime folders are private (0700), as the
    /// XDG spec requires.
    pub fn new() -> io::Result<Self> {
        let dir = tempfile::Builder::new().prefix("priya-home-").tempdir()?;
        let root = dir.path();
        let paths = PriyaPaths {
            config: root.join("priya/config"),
            data: root.join("priya/data"),
            runtime: root.join("priya/runtime"),
            state: root.join("priya/state"),
        };
        let home = root.join("home");
        let xdg = [
            ("XDG_CONFIG_HOME", home.join(".config")),
            ("XDG_DATA_HOME", home.join(".local/share")),
            ("XDG_STATE_HOME", home.join(".local/state")),
            ("XDG_CACHE_HOME", home.join(".cache")),
            ("XDG_RUNTIME_DIR", root.join("runtime")),
        ];
        for folder in [&paths.config, &paths.data, &paths.state]
            .into_iter()
            .chain(xdg[..4].iter().map(|(_, p)| p))
        {
            fs::create_dir_all(folder)?;
        }
        for runtime in [&paths.runtime, &xdg[4].1] {
            fs::create_dir_all(runtime)?;
            make_private(runtime)?;
        }
        Ok(Self {
            dir,
            paths,
            home,
            xdg,
        })
    }

    /// The temporary folder holding everything else.
    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// The PriyaOS folders.
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

    /// Exports [`env`](Self::env), plus a temporary `HOME` and `XDG_*`
    /// folders, to a child process (priyad, the CLI…).
    pub fn apply<'c>(&self, command: &'c mut Command) -> &'c mut Command {
        command
            .envs(self.env())
            .env("HOME", &self.home)
            .envs(self.xdg.iter().map(|(name, path)| (*name, path)))
    }
}

#[cfg(unix)]
fn make_private(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn make_private(_: &Path) -> io::Result<()> {
    Ok(())
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
        let root = home.root().to_path_buf();
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
        let envs: Vec<(String, std::path::PathBuf)> = command
            .get_envs()
            .map(|(k, v)| (k.to_string_lossy().into_owned(), v.unwrap().into()))
            .collect();
        assert_eq!(envs.len(), 10, "4 PRIYA_*, HOME and 5 XDG_*");
        assert!(
            envs.iter().all(|(_, path)| path.starts_with(&root)),
            "{envs:?}"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&home.paths().runtime)
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700);
        }

        drop(home);
        assert!(!root.exists(), "the folders are removed on drop");
    }
}
