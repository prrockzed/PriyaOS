//! `cargo xtask ci-local`: the CI checks that make sense on a laptop, in the
//! same order CI runs them, so a green run here predicts a green PR.

use std::path::Path;

use anyhow::Result;

use crate::cmd::{UNIT_TESTS, cargo, nextest, run, tool};
use crate::{fixtures, licenses, scope, trace};

pub fn run_all(root: &Path, fast: bool) -> Result<()> {
    let scope = scope::for_this_os(root)?;

    run("format", cargo(root).args(["fmt", "--all", "--check"]))?;

    let mut clippy = cargo(root);
    clippy.arg("clippy").args(&scope).arg("--locked");
    if !fast {
        clippy.arg("--all-targets");
    }
    run("clippy", clippy.args(["--", "-D", "warnings"]))?;

    let mut tests = nextest(root);
    tests.args(&scope);
    if fast {
        // A filter may legitimately match nothing in some crates.
        tests.arg("--no-tests=pass").args(UNIT_TESTS);
    } else {
        tests.arg("--no-tests=warn");
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

    // Without results: those come from CI's test jobs.
    println!("\n==> traceability");
    trace::run_trace(
        root,
        &trace::Options {
            junit: Vec::new(),
            base: None,
            pr_body: String::new(),
            script_results: None,
        },
    )?;

    println!("\n==> fixtures");
    fixtures::check(root)?;

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
