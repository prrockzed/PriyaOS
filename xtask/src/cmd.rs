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

/// `cargo nextest run`. nextest is required (scripts/dev-check.sh checks it);
/// it runs each test in its own process and reports better than `cargo test`.
pub fn nextest(root: &Path) -> Command {
    let mut command = cargo(root);
    command.args(["nextest", "run"]);
    command
}

/// nextest filter for unit tests: those inside library and binary targets,
/// not the integration tests under `tests/`. (`--lib` fails on a crate that
/// has only a binary.)
pub const UNIT_TESTS: [&str; 2] = ["-E", "kind(lib) | kind(bin)"];

/// A command that runs in the workspace root.
pub fn tool(root: &Path, program: &str) -> Command {
    let mut command = Command::new(program);
    command.current_dir(root);
    command
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
