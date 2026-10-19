//! PriyaOS developer commands: `cargo xtask <command>`.
//!
//! Sub-phases add commands as they need them (test suites and coverage in
//! 0.4, schemas in 0.9, the gallery in 0.11, the ISO in 2.5).

#![allow(clippy::print_stdout, clippy::print_stderr)] // a CLI prints

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cargo xtask", about = "PriyaOS developer commands")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run CI's checks locally: formatting, clippy, Rust tests, script tests.
    /// On macOS only the portable crates are checked.
    CiLocal {
        /// Quicker: clippy skips test code, and only library unit tests run.
        #[arg(long)]
        fast: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Cmd::CiLocal { fast } => ci_local(fast),
    }
}

fn ci_local(fast: bool) -> Result<()> {
    let root = workspace_root()?;
    let scope = package_scope(&root)?;

    run("format", cargo(&root).args(["fmt", "--all", "--check"]))?;

    let mut clippy = cargo(&root);
    clippy.arg("clippy").args(&scope);
    if !fast {
        clippy.arg("--all-targets");
    }
    run("clippy", clippy.args(["--", "-D", "warnings"]))?;

    let mut tests = cargo(&root);
    if has_nextest(&root) {
        tests.args(["nextest", "run", "--no-tests=warn"]);
    } else {
        tests.arg("test");
    }
    tests.args(&scope);
    if fast {
        tests.arg("--lib");
    }
    run("tests", &mut tests)?;

    run(
        "script tests",
        Command::new("bash")
            .arg("scripts/tests/run.sh")
            .current_dir(&root),
    )?;

    println!("\nAll checks passed.");
    Ok(())
}

/// `--workspace` on Linux; on macOS, `-p` for each crate listed in
/// `[workspace.metadata.priya] portable`, since the rest may need Linux.
fn package_scope(root: &Path) -> Result<Vec<String>> {
    if !cfg!(target_os = "macos") {
        return Ok(vec!["--workspace".into()]);
    }
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

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask must live one level below the workspace root")
}

fn cargo(root: &Path) -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command.current_dir(root);
    command
}

fn has_nextest(root: &Path) -> bool {
    cargo(root)
        .args(["nextest", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

fn run(name: &str, command: &mut Command) -> Result<()> {
    println!("\n==> {name}");
    let status = command
        .status()
        .with_context(|| format!("starting {name}"))?;
    if !status.success() {
        bail!("{name} failed");
    }
    Ok(())
}
