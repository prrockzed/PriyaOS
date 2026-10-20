//! Qt Quick Test files (`tst_*.qml`), run with `qmltestrunner` on the
//! offscreen platform (sub-phase 0.11). Each file runs once with exactly the
//! selected functions; the outcomes go to `target/trace/ui-results.json` for
//! `cargo xtask trace`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::tags::{Runner, TestCase};
use crate::trace::results::{self, Outcome, Results, UI_RESULTS};

/// Where Arch installs Qt's developer tools (they aren't on PATH there).
const ARCH_QT_BIN: &str = "/usr/lib/qt6/bin";
/// Overrides where Qt's tools are looked for.
const QT_BIN_VAR: &str = "PRIYA_QT_BIN";

/// A Qt tool such as `qmltestrunner`: from `$PRIYA_QT_BIN`, else Arch's
/// Qt folder, else PATH (Homebrew on macOS).
pub fn qt_tool(name: &str) -> PathBuf {
    if let Some(dir) = std::env::var_os(QT_BIN_VAR) {
        return Path::new(&dir).join(name);
    }
    let arch = Path::new(ARCH_QT_BIN).join(name);
    if arch.is_file() {
        return arch;
    }
    PathBuf::from(name)
}

/// `qmltestrunner` for one file, importing `ui/` so `import PriyaUI` works.
fn runner(root: &Path, file: &str) -> Command {
    let mut command = Command::new(qt_tool("qmltestrunner"));
    command
        .current_dir(root)
        .env("QT_QPA_PLATFORM", "offscreen")
        .args(["-import", "ui", "-input", file]);
    command
}

/// Runs the given QML tests, one qmltestrunner call per file, and records
/// every outcome. Fails if any test failed or a file couldn't run.
pub fn run_tests(root: &Path, tests: &[&TestCase]) -> Result<()> {
    let mut by_file: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for test in tests {
        if let Runner::Qml { function } = &test.runner {
            by_file.entry(&test.file).or_default().push(function);
        }
    }
    let mut failed = Vec::new();
    for (file, functions) in by_file {
        println!("\n==> {file}");
        let outcome = run_file(root, file, &functions);
        let results = match &outcome {
            Ok(results) => results.clone(),
            // A file that can't load fails every test in it.
            Err(_) => functions
                .iter()
                .map(|f| (format!("{file}::{f}"), Outcome::Failed))
                .collect(),
        };
        // The tests' own result matters more than recording it.
        if let Err(e) = results::record(&root.join(UI_RESULTS), &results) {
            eprintln!("warning: couldn't record the results of {file}: {e:#}");
        }
        match outcome {
            Err(e) => failed.push(format!("{e:#}")),
            Ok(results) => failed.extend(
                results
                    .iter()
                    .filter(|(_, o)| **o == Outcome::Failed)
                    .map(|(id, _)| format!("{id} failed")),
            ),
        }
    }
    if failed.is_empty() {
        return Ok(());
    }
    for failure in &failed {
        eprintln!("  {failure}");
    }
    bail!("{} QML test failure(s)", failed.len());
}

/// Runs `functions` of one file and returns their outcomes by test id.
fn run_file(root: &Path, file: &str, functions: &[&str]) -> Result<Results> {
    let listing = runner(root, file)
        .arg("-functions")
        .output()
        .with_context(|| format!("starting qmltestrunner (set {QT_BIN_VAR} to Qt's bin folder)"))?;
    // qmltestrunner prints the list on stderr.
    let listing = String::from_utf8_lossy(&listing.stderr).into_owned()
        + &String::from_utf8_lossy(&listing.stdout);
    let qualified = qualify(&listing, functions).with_context(|| format!("listing {file}"))?;

    let report = tempfile::NamedTempFile::new().context("creating a JUnit file")?;
    let status = runner(root, file)
        .arg("-o")
        .arg(format!("{},junitxml", report.path().display()))
        .args(["-o", "-,txt"])
        .args(&qualified)
        .stdin(Stdio::null())
        .status()
        .context("starting qmltestrunner")?;
    let junit = std::fs::read_to_string(report.path()).unwrap_or_default();
    let results = outcomes(&junit, file, functions)?;
    if !status.success() && !results.values().any(|o| *o == Outcome::Failed) {
        bail!("qmltestrunner failed on {file} without a failing test (does it load?)");
    }
    Ok(results)
}

/// Each function's `TestCaseName::function` from `qmltestrunner -functions`
/// output, which is how it selects tests. A function it doesn't list would
/// never run, so that is an error.
pub fn qualify(listing: &str, functions: &[&str]) -> Result<Vec<String>> {
    let listed: Vec<&str> = listing
        .lines()
        .filter_map(|l| l.trim().strip_suffix("()"))
        .filter(|l| l.contains("::"))
        .collect();
    let mut qualified = Vec::new();
    let mut missing = Vec::new();
    for function in functions {
        let mut matches = listed
            .iter()
            .filter(|l| l.ends_with(&format!("::{function}")));
        match (matches.next(), matches.next()) {
            (Some(name), None) => qualified.push((*name).to_owned()),
            (Some(_), Some(_)) => bail!("{function} is in more than one TestCase; rename one"),
            (None, _) => missing.push(*function),
        }
    }
    if !missing.is_empty() {
        bail!(
            "qmltestrunner doesn't list {} (does the file load?)",
            missing.join(", ")
        );
    }
    Ok(qualified)
}

/// The outcome of each of `functions` in qmltestrunner's JUnit report, keyed
/// by test id (`<file>::<function>`). One the report doesn't mention failed:
/// the run stopped before it.
pub fn outcomes(junit: &str, file: &str, functions: &[&str]) -> Result<Results> {
    let reported = if junit.trim().is_empty() {
        Results::new()
    } else {
        results::junit(junit)?
    };
    Ok(functions
        .iter()
        .map(|function| {
            // Ids are `qmltestrunner::<TestCase>::<function>`.
            let outcome = reported
                .iter()
                .find(|(id, _)| id.ends_with(&format!("::{function}")))
                .map_or(Outcome::Failed, |(_, o)| *o);
            (format!("{file}::{function}"), outcome)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-09
    // suite: unit
    #[test]
    fn qml_functions_are_qualified_from_the_listing() {
        let listing = "Theme::test_a()\nTheme::test_b()\nqml: noise\n";
        assert_eq!(
            qualify(listing, &["test_b"]).unwrap(),
            ["Theme::test_b"],
            "only the selected function"
        );
        let err = qualify(listing, &["test_c"]).unwrap_err().to_string();
        assert!(err.contains("doesn't list test_c"), "{err}");
        let twice = "A::test_a()\nB::test_a()\n";
        assert!(qualify(twice, &["test_a"]).is_err());
        // A prefix of another name doesn't count.
        assert!(qualify("T::test_ab()\n", &["test_b"]).is_err());
    }

    // covers: QA-09
    // suite: unit
    #[test]
    fn qml_junit_outcomes_map_to_test_ids() {
        let junit = r#"<?xml version="1.0" encoding="UTF-8" ?>
<testsuite name="qmltestrunner" tests="4" failures="1" errors="0" skipped="1">
  <testcase name="X::initTestCase" classname="qmltestrunner" time="0.000"/>
  <testcase name="X::test_a" classname="qmltestrunner"><failure type="fail" message="no"/></testcase>
  <testcase name="X::test_b" classname="qmltestrunner"><skipped message="no"/></testcase>
  <testcase name="X::test_c" classname="qmltestrunner" time="0.000"/>
</testsuite>"#;
        let results =
            outcomes(junit, "ui/tests/tst_x.qml", &["test_a", "test_b", "test_c"]).unwrap();
        assert_eq!(results["ui/tests/tst_x.qml::test_a"], Outcome::Failed);
        assert_eq!(results["ui/tests/tst_x.qml::test_b"], Outcome::Skipped);
        assert_eq!(results["ui/tests/tst_x.qml::test_c"], Outcome::Passed);
        let none = outcomes("", "f.qml", &["test_a"]).unwrap();
        assert_eq!(none["f.qml::test_a"], Outcome::Failed, "never reported");
    }

    // Linux only: the CI image always has Qt, the macOS runner doesn't.
    // covers: QA-09
    // suite: ui
    #[cfg(target_os = "linux")]
    #[test]
    fn qml_tests_run_offscreen_and_record_results() {
        use crate::tags::Tags;
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("ui/tests");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("tst_demo.qml"),
            "import QtQuick\nimport QtTest\nTestCase {\n    name: \"Demo\"\n    function test_passes() { compare(1 + 1, 2) }\n    function test_fails() { compare(1, 2) }\n    function test_unselected() { fail(\"never runs\") }\n}\n",
        )
        .unwrap();
        let case = |function: &str| TestCase {
            id: format!("ui/tests/tst_demo.qml::{function}"),
            file: "ui/tests/tst_demo.qml".into(),
            line: 1,
            conditional: false,
            ignored: None,
            tags: Tags::default(),
            runner: Runner::Qml {
                function: function.into(),
            },
        };
        let (passes, fails) = (case("test_passes"), case("test_fails"));

        run_tests(root.path(), &[&passes]).unwrap();
        let err = run_tests(root.path(), &[&passes, &fails]).unwrap_err();
        assert!(err.to_string().contains("1 QML test failure"), "{err:#}");

        let recorded = results::read_file(&root.path().join(UI_RESULTS)).unwrap();
        assert_eq!(recorded.len(), 2, "test_unselected never ran: {recorded:?}");
        assert_eq!(recorded[&passes.id], Outcome::Passed);
        assert_eq!(recorded[&fails.id], Outcome::Failed);

        // A file that doesn't load fails every selected test in it.
        std::fs::write(dir.join("tst_demo.qml"), "TestCase {").unwrap();
        assert!(run_tests(root.path(), &[&passes]).is_err());
        let recorded = results::read_file(&root.path().join(UI_RESULTS)).unwrap();
        assert_eq!(recorded[&passes.id], Outcome::Failed);
    }
}
