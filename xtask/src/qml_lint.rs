//! qmllint for the UI (0.11, 0.13): which QML files are checked, and
//! `cargo xtask ui qmllint`, which lints all of them. The pre-commit hook
//! lints the staged ones with the same rules.
//!
//! The shell (`ui/shell/`) imports Quickshell, which only Linux has. Where
//! it isn't installed (the Mac), its files are left to CI, whose image has
//! it; CI passes `--require-quickshell` so they can't be skipped there.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::qml::qt_tool;

/// QML that is checked: not the generated Theme.qml, not lint fixtures
/// (which are wrong on purpose).
pub fn is_checked(path: &str) -> bool {
    path.ends_with(".qml") && path != "ui/PriyaUI/Theme.qml" && !path.contains("/fixtures/")
}

/// QML that needs Quickshell to resolve its imports.
pub fn needs_quickshell(path: &str) -> bool {
    path.starts_with("ui/shell/")
}

/// Whether Quickshell's QML module is installed next to Qt's.
pub fn quickshell_installed() -> bool {
    quickshell_dir().is_ok_and(|dir| dir.is_dir())
}

/// Where Quickshell's QML module would be, from `qtpaths`.
fn quickshell_dir() -> Result<PathBuf> {
    let qtpaths = qt_tool("qtpaths");
    let out = Command::new(&qtpaths)
        .args(["--query", "QT_INSTALL_QML"])
        .output()
        .with_context(|| format!("starting {}", qtpaths.display()))?;
    let qml = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !out.status.success() || qml.is_empty() {
        bail!(
            "{} didn't say where Qt's QML modules are: {}",
            qtpaths.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(Path::new(&qml).join("Quickshell"))
}

/// qmllint's warnings for `path` (relative to `repo`), or `None` when it
/// is clean. A singleton linted alone is reported as undeclared, so
/// singletons are skipped; their users are linted with the module's qmldir.
pub fn lint(repo: &Path, path: &str, source: &str) -> Result<Option<String>> {
    if source.lines().any(|l| l.trim() == "pragma Singleton") {
        return Ok(None);
    }
    let lint = Command::new(qt_tool("qmllint"))
        .current_dir(repo)
        .args(["-W", "0", "-I", "ui", path])
        .output()
        .context("starting qmllint (Qt's declarative tools)")?;
    Ok((!lint.status.success())
        .then(|| String::from_utf8_lossy(&lint.stderr).trim_end().to_owned()))
}

/// `cargo xtask ui qmllint`: every checked QML file under ui/.
pub fn check_all(repo: &Path, require_quickshell: bool) -> Result<()> {
    let mut files = Vec::new();
    collect(&repo.join("ui"), repo, &mut files)?;
    files.sort();
    let quickshell = quickshell_installed();
    if require_quickshell && !quickshell {
        let dir = quickshell_dir()?;
        bail!(
            "Quickshell isn't installed ({} is missing), so ui/shell can't be linted",
            dir.display()
        );
    }
    let mut problems = Vec::new();
    let mut skipped = 0;
    for path in &files {
        if needs_quickshell(path) && !quickshell {
            skipped += 1;
            continue;
        }
        let source =
            std::fs::read_to_string(repo.join(path)).with_context(|| format!("reading {path}"))?;
        if let Some(warnings) = lint(repo, path, &source)? {
            problems.push(format!("{path}:\n{warnings}"));
        }
    }
    if skipped > 0 {
        println!("{skipped} ui/shell files left to CI: Quickshell isn't installed here.");
    }
    if problems.is_empty() {
        println!("qmllint: {} files, no warnings.", files.len() - skipped);
        return Ok(());
    }
    for problem in &problems {
        eprintln!("{problem}\n");
    }
    bail!("qmllint found warnings in {} files", problems.len());
}

fn collect(dir: &Path, repo: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path: PathBuf = entry?.path();
        if path.is_dir() {
            collect(&path, repo, out)?;
        } else if let Ok(relative) = path.strip_prefix(repo) {
            let relative = relative.to_string_lossy().replace('\\', "/");
            if is_checked(&relative) {
                out.push(relative);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-09
    // suite: unit
    #[test]
    fn checked_qml_leaves_out_generated_and_fixture_files_and_spots_the_shell() {
        assert!(is_checked("ui/PriyaUI/Button.qml"));
        assert!(is_checked("ui/tests/components/tst_button.qml"));
        assert!(!is_checked("ui/PriyaUI/Theme.qml"));
        assert!(!is_checked("ui/tests/lint/fixtures/BadNames.qml"));
        assert!(!is_checked("ui/PriyaUI/MarkdownBlocks.js"));
        assert!(is_checked("ui/shell/priya/PriyaBar.qml"));
        assert!(needs_quickshell("ui/shell/priya/shell.qml"));
        assert!(!needs_quickshell("ui/PriyaUI/PriyaBarContent.qml"));
    }

    // covers: QA-09
    // suite: unit
    #[test]
    fn every_checked_qml_file_under_ui_is_found_in_order() {
        let repo = repo_with(&[
            ("ui/shell/priya/shell.qml", ""),
            ("ui/PriyaUI/Card.qml", ""),
            ("ui/PriyaUI/Theme.qml", ""),
            ("ui/PriyaUI/qmldir", ""),
            ("ui/tests/lint/fixtures/Bad.qml", ""),
            ("ui/tests/components/tst_card.qml", ""),
        ]);
        let mut found = Vec::new();
        collect(&repo.path().join("ui"), repo.path(), &mut found).unwrap();
        found.sort();
        assert_eq!(
            found,
            [
                "ui/PriyaUI/Card.qml",
                "ui/shell/priya/shell.qml",
                "ui/tests/components/tst_card.qml"
            ]
        );
    }

    /// A throwaway repository with these files under it.
    fn repo_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        for (file, text) in files {
            let path = repo.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        repo
    }

    // Linux only: the CI image has Qt's tools, the macOS runner doesn't.
    // covers: QA-09
    // suite: unit
    #[cfg(target_os = "linux")]
    #[test]
    fn clean_qml_passes_and_a_broken_file_is_named() {
        let good = "import QtQuick\n\nItem {\n    width: 1\n}\n";
        let singleton = "pragma Singleton\n\nimport QtQuick\n\nQtObject {}\n";
        let repo = repo_with(&[
            ("ui/PriyaUI/Good.qml", good),
            ("ui/PriyaUI/Single.qml", singleton),
        ]);
        check_all(repo.path(), false).expect("clean QML passes");
        std::fs::write(
            repo.path().join("ui/PriyaUI/Bad.qml"),
            "import QtQuick\n\nItem {\n    noSuchProperty: 1\n}\n",
        )
        .unwrap();
        let failed = check_all(repo.path(), false).unwrap_err().to_string();
        assert!(failed.contains("1 files"), "{failed}");
    }

    // The CI image has Quickshell, so the shell is linted there.
    // covers: QA-09
    // suite: unit
    #[cfg(target_os = "linux")]
    #[test]
    fn the_shell_is_linted_where_quickshell_is_installed() {
        if !quickshell_installed() {
            // A Linux machine without Quickshell can't run this one.
            assert!(check_all(Path::new("."), true).is_err());
            return;
        }
        let repo = repo_with(&[(
            "ui/shell/priya/shell.qml",
            "import Quickshell\n\nShellRoot {}\n",
        )]);
        check_all(repo.path(), true).expect("Quickshell's types resolve");
    }
}
