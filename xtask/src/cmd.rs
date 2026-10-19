//! Small helpers for finding the workspace and running tools.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask must live one level below the workspace root")
}

/// `cargo` from the `CARGO` variable, so nested calls use the same toolchain.
pub fn cargo(root: &Path) -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command.current_dir(root);
    command
}

/// A command that runs in the workspace root.
pub fn tool(root: &Path, program: &str) -> Command {
    let mut command = Command::new(program);
    command.current_dir(root);
    command
}

/// True when the command starts and exits successfully, e.g. `x --version`.
pub fn succeeds(command: &mut Command) -> bool {
    command.output().is_ok_and(|o| o.status.success())
}

/// Runs a step with a heading, failing if the command fails or can't start.
pub fn run(name: &str, command: &mut Command) -> Result<()> {
    println!("\n==> {name}");
    let status = command.status().with_context(|| {
        format!("starting {name}; scripts/dev-check.sh lists the tools that are missing")
    })?;
    if !status.success() {
        bail!("{name} failed");
    }
    Ok(())
}
