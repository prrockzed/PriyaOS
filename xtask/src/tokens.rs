//! `cargo xtask tokens [--check]`: generates `ui/PriyaUI/Theme.qml` and
//! `ui/design/build/tokens.css` from `ui/design/tokens.json`, and writes
//! the contrast report to `target/design/contrast.md`. Fails when any
//! declared pair is below its contrast threshold. With `--check`, also
//! fails when a committed file differs from what the tokens generate.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

const TOKENS: &str = "ui/design/tokens.json";
const THEME_QML: &str = "ui/PriyaUI/Theme.qml";
const TOKENS_CSS: &str = "ui/design/build/tokens.css";
// Under the workspace, not CARGO_TARGET_DIR: CI uploads it from here.
const REPORT: &str = "target/design/contrast.md";

pub fn run(root: &Path, check: bool) -> Result<()> {
    let tokens = priya_themegen::load(&root.join(TOKENS))?;
    let outputs = priya_themegen::generate(&tokens)?;
    write(&root.join(REPORT), &outputs.contrast.to_markdown())?;
    if !outputs.contrast.passes() {
        for problem in outputs.contrast.problems() {
            eprintln!("  {problem}");
        }
        bail!("colours fail the contrast check; the report is in {REPORT}");
    }

    let generated = [
        (THEME_QML, outputs.theme_qml),
        (TOKENS_CSS, outputs.tokens_css),
    ];
    if check {
        let stale = stale(root, &generated)?;
        if !stale.is_empty() {
            for path in &stale {
                eprintln!("  {path} is out of date");
            }
            bail!("generated files differ from {TOKENS}; run `cargo xtask tokens` and commit");
        }
        println!("The generated theme files match {TOKENS}, and every contrast pair passes.");
    } else {
        for (path, text) in &generated {
            write(&root.join(path), text)?;
        }
        println!("Generated {THEME_QML} and {TOKENS_CSS}; contrast report in {REPORT}.");
    }
    Ok(())
}

/// The generated files whose committed text differs (or is missing).
fn stale<'a>(root: &Path, generated: &'a [(&'a str, String)]) -> Result<Vec<&'a str>> {
    let mut stale = Vec::new();
    for (path, text) in generated {
        let full: PathBuf = root.join(path);
        let committed = match fs::read_to_string(&full) {
            Ok(committed) => committed,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e).with_context(|| format!("reading {path}")),
        };
        if committed != *text {
            stale.push(*path);
        }
    }
    Ok(stale)
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: DS-01
    // suite: unit
    #[test]
    fn check_finds_stale_and_missing_files() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("ui/PriyaUI")).unwrap();
        fs::write(root.path().join(THEME_QML), "same").unwrap();
        let generated = [
            (THEME_QML, "same".to_owned()),
            (TOKENS_CSS, "new".to_owned()),
        ];
        assert_eq!(stale(root.path(), &generated).unwrap(), [TOKENS_CSS]);
        write(&root.path().join(TOKENS_CSS), "new").unwrap();
        assert!(stale(root.path(), &generated).unwrap().is_empty());
    }
}
