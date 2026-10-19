//! Which crates a check covers. Linux checks everything; macOS checks only
//! the crates listed in `[workspace.metadata.priya] portable`, since the
//! rest may need Linux.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::cmd::cargo;

/// The workspace as `cargo metadata` describes it.
pub struct Workspace {
    pub packages: Vec<Package>,
    /// `[workspace.metadata.priya] portable`, if the workspace declares it.
    portable: Option<Vec<String>>,
}

pub struct Package {
    pub name: String,
    /// The folder holding the package's Cargo.toml.
    pub dir: PathBuf,
    pub targets: Vec<Target>,
}

/// One compile target of a package, as cargo metadata reports it.
pub struct Target {
    /// `lib`, `bin`, `test`, `bench`, `example`, `proc-macro`, `custom-build`…
    pub kind: String,
    pub name: String,
    /// The target's root file (`src/lib.rs`, `src/main.rs`, `tests/x.rs`).
    pub src_path: PathBuf,
}

impl Workspace {
    pub fn load(root: &Path) -> Result<Self> {
        let output = cargo(root)
            .args(["metadata", "--format-version", "1", "--no-deps"])
            .output()
            .context("running cargo metadata")?;
        if !output.status.success() {
            bail!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&output.stdout).context("reading cargo metadata")?;

        let mut packages = Vec::new();
        for package in metadata["packages"].as_array().into_iter().flatten() {
            let name = package["name"].as_str().context("package without a name")?;
            let manifest = package["manifest_path"]
                .as_str()
                .context("package without a manifest path")?;
            let dir = Path::new(manifest)
                .parent()
                .context("manifest path has no folder")?;
            let mut targets = Vec::new();
            for target in package["targets"].as_array().into_iter().flatten() {
                let kind = target["kind"][0]
                    .as_str()
                    .context("target without a kind")?;
                let name = target["name"].as_str().context("target without a name")?;
                let src_path = target["src_path"]
                    .as_str()
                    .context("target without a source path")?;
                targets.push(Target {
                    kind: kind.to_owned(),
                    name: name.to_owned(),
                    src_path: PathBuf::from(src_path),
                });
            }
            packages.push(Package {
                name: name.to_owned(),
                dir: dir.to_path_buf(),
                targets,
            });
        }

        let portable = match &metadata["metadata"]["priya"]["portable"] {
            serde_json::Value::Null => None,
            list => {
                let list = list
                    .as_array()
                    .context("[workspace.metadata.priya] portable must be a list")?;
                let names = list
                    .iter()
                    .map(|n| n.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
                    .context("portable entries must be crate names")?;
                Some(names)
            }
        };
        Ok(Self { packages, portable })
    }

    /// A workspace made by hand, for tests.
    #[cfg(test)]
    pub fn from_packages(packages: Vec<Package>) -> Self {
        Self {
            packages,
            portable: None,
        }
    }

    /// The package whose folder holds `path`, the innermost one if packages
    /// nest.
    pub fn package_of(&self, path: &Path) -> Option<&Package> {
        self.packages
            .iter()
            .filter(|p| path.starts_with(&p.dir))
            .max_by_key(|p| p.dir.components().count())
    }

    /// The crates that must build on macOS. A workspace that doesn't say is
    /// taken to be portable throughout.
    pub fn portable_names(&self) -> Vec<String> {
        match &self.portable {
            Some(names) => names.clone(),
            None => self.packages.iter().map(|p| p.name.clone()).collect(),
        }
    }

    /// Drops crates this OS can't build: on macOS, the non-portable ones.
    pub fn buildable_here(&self, names: Vec<String>) -> Vec<String> {
        if !cfg!(target_os = "macos") {
            return names;
        }
        let portable = self.portable_names();
        names.into_iter().filter(|n| portable.contains(n)).collect()
    }
}

/// `--workspace` on Linux, the portable crates on macOS.
pub fn for_this_os(root: &Path) -> Result<Vec<String>> {
    if cfg!(target_os = "macos") {
        portable(root)
    } else {
        Ok(vec!["--workspace".into()])
    }
}

/// `-p <crate>` for each portable crate.
pub fn portable(root: &Path) -> Result<Vec<String>> {
    let workspace = Workspace::load(root)?;
    Ok(package_args(&workspace.portable_names()))
}

/// `-p a -p b …` for cargo.
pub fn package_args(names: &[String]) -> Vec<String> {
    names
        .iter()
        .flat_map(|name| ["-p".to_owned(), name.clone()])
        .collect()
}
