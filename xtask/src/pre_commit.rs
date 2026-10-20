//! `cargo xtask pre-commit`: fast checks on what is about to be committed,
//! run by `.githooks/pre-commit`. Budget: under 30 s on the Zenbook.
//!
//! - Formatting is checked on the staged content of each Rust file, so a
//!   formatted working copy can't hide an unformatted commit.
//! - Clippy and unit tests run on the crates the staged files belong to.
//!   They see the working tree, which is usually the same as the index.
//! - Staged QML must be as `qmlformat` writes it and pass `qmllint` with
//!   no warnings (0.11).
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
use crate::qml::qt_tool;
use crate::qml_lint;
use crate::scope::{Workspace, package_args};
use crate::trace;

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

    let qml: Vec<&String> = staged.iter().filter(|p| qml_lint::is_checked(p)).collect();
    if !qml.is_empty() {
        println!("\n==> QML (qmlformat, qmllint)");
        check_staged_qml(&repo, &qml)?;
    }

    if staged.iter().any(|p| is_test_source(p)) && repo.join(trace::FEATURES).exists() {
        println!("\n==> test tags");
        trace::check_tags(&repo)?;
    }

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

/// Formatting on the staged content; lint on the working copy, which
/// qmllint needs to resolve imports next to it.
fn check_staged_qml(repo: &Path, files: &[&String]) -> Result<()> {
    let dir = tempfile::tempdir().context("making a temporary folder")?;
    let mut problems = Vec::new();
    let quickshell = qml_lint::quickshell_installed();
    if !quickshell && files.iter().any(|p| qml_lint::needs_quickshell(p)) {
        println!("Quickshell isn't installed: CI lints the staged ui/shell files.");
    }
    for path in files {
        let staged = tool(repo, "git")
            .args(["show", &format!(":{path}")])
            .output()
            .with_context(|| format!("reading the staged {path}"))?;
        // Same name, so qmlformat's messages name the right file.
        let name = Path::new(path.as_str()).file_name().unwrap_or_default();
        let copy = dir.path().join(name);
        std::fs::write(&copy, &staged.stdout).context("writing a temporary copy")?;
        let formatted = Command::new(qt_tool("qmlformat"))
            .arg(&copy)
            .output()
            .context("starting qmlformat (Qt's declarative tools)")?;
        if !formatted.status.success() {
            problems.push(format!("qmlformat couldn't parse {path}"));
        } else if formatted.stdout != staged.stdout {
            problems.push(format!("not formatted: {path} (run qmlformat -i {path})"));
        }
        if qml_lint::needs_quickshell(path) && !quickshell {
            continue;
        }
        let source = String::from_utf8_lossy(&staged.stdout);
        if let Some(warnings) = qml_lint::lint(repo, path, &source)? {
            problems.push(format!("qmllint warnings in {path}:\n{warnings}"));
        }
    }
    if problems.is_empty() {
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  {problem}");
    }
    bail!("fix the QML above, then stage it");
}

/// Files that may hold tagged tests.
fn is_test_source(path: &str) -> bool {
    [".rs", ".qml", ".py", ".sh"]
        .iter()
        .any(|ext| path.ends_with(ext))
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

    // Linux only: the CI image has Qt's tools, the macOS runner doesn't.
    // covers: QA-03
    // suite: unit
    #[cfg(target_os = "linux")]
    #[test]
    fn hook_rejects_unformatted_or_warning_qml() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let git = |args: &[&str]| {
            assert!(
                crate::cmd::git(repo)
                    .args(args)
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
        };
        git(&["init", "-q"]);
        std::fs::create_dir_all(repo.join("ui")).unwrap();
        let file = "ui/A.qml".to_owned();
        let stage = |text: &str| {
            std::fs::write(repo.join(&file), text).unwrap();
            git(&["add", "ui/A.qml"]);
        };
        stage("import QtQuick\n\nItem {\n    width: 2\n}\n");
        check_staged_qml(repo, &[&file]).unwrap();
        stage("import QtQuick\nItem {   width: 2 }\n");
        let err = check_staged_qml(repo, &[&file]).unwrap_err().to_string();
        assert!(err.contains("fix the QML"), "{err}");
        stage("import QtQuick\n\nItem {\n    width: missing\n}\n");
        assert!(
            check_staged_qml(repo, &[&file]).is_err(),
            "a qmllint warning fails"
        );
    }

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
