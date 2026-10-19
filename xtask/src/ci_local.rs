//! `cargo xtask ci-local`: the CI checks that make sense on a laptop, in the
//! same order CI runs them, so a green run here predicts a green PR.

use std::path::Path;

use anyhow::Result;

use crate::cmd::{cargo, run, succeeds, tool};
use crate::{licenses, scope};

pub fn run_all(root: &Path, fast: bool) -> Result<()> {
    let scope = scope::for_this_os(root)?;

    run("format", cargo(root).args(["fmt", "--all", "--check"]))?;

    let mut clippy = cargo(root);
    clippy.arg("clippy").args(&scope).arg("--locked");
    if !fast {
        clippy.arg("--all-targets");
    }
    run("clippy", clippy.args(["--", "-D", "warnings"]))?;

    let mut tests = cargo(root);
    if succeeds(cargo(root).args(["nextest", "--version"])) {
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
        "shellcheck",
        tool(root, "bash").arg("scripts/lint-shell.sh"),
    )?;
    run(
        "script tests",
        tool(root, "bash").arg("scripts/tests/run.sh"),
    )?;

    println!("\n==> third-party licences");
    licenses::check(root)?;
    // Advisories need the network, so they stay in CI only.
    run(
        "cargo deny",
        // Errors only: CI shows the warnings (e.g. duplicate versions) in full.
        cargo(root).args([
            "deny",
            "--log-level",
            "error",
            "check",
            "licenses",
            "bans",
            "sources",
        ]),
    )?;
    run("spelling", &mut tool(root, "typos"))?;
    run(
        "links",
        tool(root, "lychee").args([
            "--offline",
            "--include-fragments",
            "--no-progress",
            "**/*.md",
        ]),
    )?;

    println!("\nAll checks passed.");
    Ok(())
}
