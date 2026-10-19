//! `cargo xtask pre-commit`: fast checks on what is about to be committed,
//! run by `.githooks/pre-commit`. Budget: under 30 s on the Zenbook.
//!
//! - Formatting is checked on the staged content of each Rust file, so a
//!   formatted working copy can't hide an unformatted commit.
//! - Clippy and unit tests run on the crates the staged files belong to.
//!   They see the working tree, which is usually the same as the index.
//! - gitleaks scans the staged diff for secrets.
//!
//! It works in whichever repository it is started in, which lets the script
//! tests run it in a throwaway one.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::cmd::{UNIT_TESTS, cargo, nextest, run, tool};
use crate::scope::{Workspace, package_args};

/// Files that change how every crate builds or is tested. Crates that merely
/// depend on a changed crate are not re-checked here; the pre-push hook and
/// CI cover the whole workspace.
const WORKSPACE_FILES: [&str; 7] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "clippy.toml",
    "rustfmt.toml",
    ".cargo/config.toml",
    ".config/nextest.toml",
];

pub fn run_all() -> Result<()> {
    let repo = repo_root()?;
    let staged = staged_files(&repo)?;
    if staged.is_empty() {
        return Ok(());
    }

    println!("==> format (staged Rust files)");
    check_staged_formatting(&repo, &staged)?;

    let crates = changed_crates(&repo, &staged)?;
    if crates.is_empty() {
        println!("\nNo Rust crate changed; skipping clippy and unit tests.");
    } else {
        let packages = package_args(&crates);
        run(
            "clippy (changed crates)",
            cargo(&repo).arg("clippy").args(&packages).args([
                "--all-targets",
                "--locked",
                "--",
                "-D",
                "warnings",
            ]),
        )?;
        run(
            "unit tests (changed crates)",
            nextest(&repo)
                .arg("--no-tests=pass")
                .args(&packages)
                .args(UNIT_TESTS),
        )?;
    }

    run(
        "secrets (staged diff)",
        tool(&repo, "gitleaks").args([
            "git",
            "--pre-commit",
            "--staged",
            "--redact",
            "--no-banner",
            // Names the file and rule; --redact keeps the secret itself out.
            "--verbose",
            ".",
        ]),
    )?;
    Ok(())
}

fn repo_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("running git")?;
    if !output.status.success() {
        bail!("not inside a git repository");
    }
    Ok(PathBuf::from(
        String::from_utf8(output.stdout)
            .context("repository path is not UTF-8")?
            .trim_end(),
    ))
}

/// Added, copied, modified or renamed files in the index; deletions need no
/// checking.
fn staged_files(repo: &Path) -> Result<Vec<String>> {
    let output = tool(repo, "git")
        .args([
            "diff",
            "--cached",
            "--name-only",
            "--diff-filter=ACMR",
            "-z",
        ])
        .output()
        .context("listing staged files")?;
    if !output.status.success() {
        bail!("git diff --cached failed");
    }
    let names = String::from_utf8(output.stdout).context("a staged path is not UTF-8")?;
    Ok(names
        .split('\0')
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .collect())
}

fn check_staged_formatting(repo: &Path, staged: &[String]) -> Result<()> {
    let mut problems = Vec::new();
    for path in staged.iter().filter(|p| p.ends_with(".rs")) {
        let content = tool(repo, "git")
            .args(["show", &format!(":{path}")])
            .output()
            .with_context(|| format!("reading the staged {path}"))?;
        if !content.status.success() {
            bail!("git show :{path} failed");
        }
        match formatting(repo, &content.stdout)? {
            Formatting::Formatted => {}
            Formatting::Unformatted => problems.push(format!("not formatted: {path}")),
            Formatting::Unparsable(error) => {
                problems.push(format!("rustfmt couldn't parse {path}:\n{error}"));
            }
        }
    }
    if problems.is_empty() {
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  {problem}");
    }
    bail!("fix the files above (`cargo fmt --all` formats the workspace), then stage them");
}

enum Formatting {
    Formatted,
    Unformatted,
    /// rustfmt's error message, usually a syntax error.
    Unparsable(String),
}

/// Formats `source` with rustfmt and compares. (`rustfmt --check` exits 0
/// on stdin even when it prints a diff, so it can't be used here.) rustfmt
/// finds `rustfmt.toml` from the repository root, its working directory.
fn formatting(repo: &Path, source: &[u8]) -> Result<Formatting> {
    let mut child = tool(repo, "rustfmt")
        // Also set in rustfmt.toml; repeated for repositories without one,
        // such as the script tests' throwaway repository.
        .args(["--edition", "2024"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting rustfmt")?;
    // rustfmt reads all of stdin before writing, so this can't deadlock.
    child
        .stdin
        .take()
        .context("rustfmt has no stdin")?
        .write_all(source)
        .context("sending source to rustfmt")?;
    let output = child.wait_with_output().context("waiting for rustfmt")?;
    Ok(if !output.status.success() {
        Formatting::Unparsable(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    } else if output.stdout == source {
        Formatting::Formatted
    } else {
        Formatting::Unformatted
    })
}

/// The crates the staged files belong to, limited on macOS to those that
/// build there.
fn changed_crates(repo: &Path, staged: &[String]) -> Result<Vec<String>> {
    let workspace = Workspace::load(repo)?;
    let touched = touched_packages(repo, &workspace, staged);
    Ok(workspace.buildable_here(touched))
}

/// Every package when a workspace-wide file is staged; otherwise the
/// package owning each staged file. Files outside every package, such as
/// docs, touch none.
fn touched_packages(repo: &Path, workspace: &Workspace, staged: &[String]) -> Vec<String> {
    if staged.iter().any(|p| WORKSPACE_FILES.contains(&p.as_str())) {
        return workspace.packages.iter().map(|p| p.name.clone()).collect();
    }
    let owners: BTreeSet<String> = staged
        .iter()
        .filter_map(|path| workspace.package_of(&repo.join(path)))
        .map(|p| p.name.clone())
        .collect();
    owners.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::Package;

    fn package(name: &str, dir: &str) -> Package {
        Package {
            name: name.into(),
            dir: Path::new("/repo").join(dir),
            targets: Vec::new(),
        }
    }

    fn touched(staged: &[&str]) -> Vec<String> {
        let workspace = Workspace::from_packages(vec![
            package("priya-core", "crates/priya-core"),
            package("priya-cli", "crates/priya-cli"),
            package("inner", "crates/priya-cli/inner"),
        ]);
        let staged: Vec<String> = staged.iter().map(|s| (*s).to_owned()).collect();
        touched_packages(Path::new("/repo"), &workspace, &staged)
    }

    // covers: QA-03
    // suite: unit
    #[test]
    fn staged_files_map_to_their_innermost_package() {
        assert_eq!(
            touched(&[
                "crates/priya-core/src/paths.rs",
                "crates/priya-core/Cargo.toml",
                "crates/priya-cli/inner/src/lib.rs",
                "docs/README.md",
            ]),
            ["inner", "priya-core"]
        );
        assert!(touched(&["docs/README.md", "crates/priya-corex/a.rs"]).is_empty());
    }

    // covers: QA-03
    // suite: unit
    #[test]
    fn a_workspace_wide_file_touches_every_package() {
        assert_eq!(
            touched(&["Cargo.lock"]),
            ["priya-core", "priya-cli", "inner"]
        );
    }
}
