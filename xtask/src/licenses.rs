//! `cargo xtask licenses`: third-party files bundled under `ui/` must be
//! listed in THIRD_PARTY.md. `cargo deny` covers Rust crates; this covers
//! everything else (icons now, anything vendored later).
//!
//! A bundle is a folder under `ui/` that holds its own licence file
//! (`LICENSE`, `LICENCE`, `COPYING` or a font's `OFL`, optionally followed by
//! an extension or suffix). THIRD_PARTY.md must name that
//! folder in backticks. Font files must sit inside a bundle, because PriyaOS
//! normally takes fonts from Arch packages instead of vendoring them.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::cmd::{files_under, relative};

const SCANNED: &str = "ui";
const NOTICE_FILE: &str = "THIRD_PARTY.md";
const FONT_EXTENSIONS: [&str; 4] = ["ttf", "otf", "woff", "woff2"];

pub fn check(root: &Path) -> Result<()> {
    let notices = fs::read_to_string(root.join(NOTICE_FILE))
        .with_context(|| format!("reading {NOTICE_FILE}"))?;
    let problems = problems(root, &notices)?;
    if problems.is_empty() {
        println!("Every bundle under {SCANNED}/ is listed in {NOTICE_FILE}.");
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  {problem}");
    }
    bail!("{} third-party licence problem(s)", problems.len());
}

/// What is wrong, one line per problem, in path order.
fn problems(root: &Path, notices: &str) -> Result<Vec<String>> {
    let files = files_under(&[root.join(SCANNED)], &[])?;
    let mut bundles: Vec<&Path> = files
        .iter()
        .filter(|f| is_licence_file(f))
        .filter_map(|f| f.parent())
        .collect();
    // A folder with both LICENSE and COPYING is still one bundle.
    bundles.sort();
    bundles.dedup();

    let mut problems = Vec::new();
    for bundle in &bundles {
        let name = relative(root, bundle);
        if !notices.contains(&format!("`{name}`")) && !notices.contains(&format!("`{name}/`")) {
            problems.push(format!(
                "{name}/ has a licence file but no entry in {NOTICE_FILE}"
            ));
        }
    }
    for file in files.iter().filter(|f| is_font(f)) {
        if !bundles.iter().any(|b| file.starts_with(b)) {
            problems.push(format!(
                "{} is a font outside a licensed bundle; use the Arch package instead",
                relative(root, file)
            ));
        }
    }
    Ok(problems)
}

/// `LICENSE`, `LICENSE.txt`, `COPYING-MIT`, `OFL.txt` (fonts) and the like,
/// but not `LicensesPage.qml`.
fn is_licence_file(path: &Path) -> bool {
    let name = file_name(path).to_ascii_uppercase();
    ["LICENSE", "LICENCE", "COPYING", "OFL"].iter().any(|stem| {
        name.strip_prefix(stem)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['.', '-', '_']))
    })
}

fn is_font(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| FONT_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        dir
    }

    // covers: QA-10
    // suite: unit
    // acceptance: 0.3
    // regression: 0.3
    #[test]
    fn licenses_check_requires_third_party_entry() {
        let dir = tree(&[
            "ui/PriyaUI/icons/LICENSE",
            "ui/PriyaUI/icons/check.svg",
            "ui/PriyaUI/Button.qml",
        ]);
        assert_eq!(
            problems(dir.path(), "nothing listed").unwrap(),
            ["ui/PriyaUI/icons/ has a licence file but no entry in THIRD_PARTY.md"]
        );
        let listed = "| Lucide | `ui/PriyaUI/icons/` | vendored | ISC |";
        assert!(problems(dir.path(), listed).unwrap().is_empty());
    }

    // covers: QA-10
    // suite: unit
    #[test]
    fn fonts_must_live_in_a_licensed_bundle() {
        let dir = tree(&["ui/shell/fonts/Inter.ttf", "ui/vendor/f/OFL/COPYING.txt"]);
        let listed = "`ui/vendor/f/OFL`";
        assert_eq!(
            problems(dir.path(), listed).unwrap(),
            [
                "ui/shell/fonts/Inter.ttf is a font outside a licensed bundle; use the Arch package instead"
            ]
        );

        let dir = tree(&["ui/vendor/f/LICENSE.md", "ui/vendor/f/sub/F.WOFF2"]);
        assert!(problems(dir.path(), "`ui/vendor/f`").unwrap().is_empty());
    }

    // covers: QA-10
    // suite: unit
    #[test]
    fn only_real_licence_files_mark_a_bundle() {
        for name in [
            "LICENSE",
            "license.md",
            "LICENCE-MIT",
            "COPYING_v2",
            "OFL.txt",
        ] {
            assert!(is_licence_file(Path::new(name)), "{name}");
        }
        for name in [
            "LicensesPage.qml",
            "LicenseDialog.qml",
            "OFLINE.txt",
            "README.md",
        ] {
            assert!(!is_licence_file(Path::new(name)), "{name}");
        }
    }

    // covers: QA-10
    // suite: unit
    #[test]
    fn the_repository_passes_the_licence_check() {
        let root = crate::cmd::workspace_root().unwrap();
        let notices = fs::read_to_string(root.join(NOTICE_FILE)).unwrap();
        assert_eq!(problems(&root, &notices).unwrap(), Vec::<String>::new());
    }
}
