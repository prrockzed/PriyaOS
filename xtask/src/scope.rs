//! Which crates a check covers. Linux checks everything; macOS checks only
//! the crates listed in `[workspace.metadata.priya] portable`, since the
//! rest may need Linux.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::cmd::cargo;

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
    let output = cargo(root)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .context("running cargo metadata")?;
    if !output.status.success() {
        bail!("cargo metadata failed");
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("reading cargo metadata")?;
    let portable = metadata["metadata"]["priya"]["portable"]
        .as_array()
        .context("[workspace.metadata.priya] portable is missing from Cargo.toml")?;
    let mut scope = Vec::new();
    for name in portable {
        let name = name
            .as_str()
            .context("portable entries must be crate names")?;
        scope.extend(["-p".to_owned(), name.to_owned()]);
    }
    Ok(scope)
}
