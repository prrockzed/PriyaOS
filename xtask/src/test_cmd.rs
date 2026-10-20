//! `cargo xtask test <suite | regression | all>`: runs the tests the index
//! selects, whatever language they are in.
//!
//! - `<suite>`: every test tagged `suite: <suite>`.
//! - `regression --fast`: tests tagged `regression:` from the unit, contract,
//!   integration and ui suites. `regression --all`: every `regression:` test.
//! - `all`: everything.
//!
//! Rust tests run through one nextest call per chunk of tests, selected
//! exactly by binary and name. Shell tests run with bash, QML tests with
//! qmltestrunner offscreen, one call per file. On macOS, Rust tests of
//! crates that only build on Linux are skipped with a note.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result, bail};

use crate::cmd::{cargo, nextest, run, tool};
use crate::qml;
use crate::scope::{Workspace, package_args};
use crate::tags::{self, Index, Runner, SUITES, TestCase};
use crate::trace::results::{self, Outcome};

/// Suites that `regression --fast` takes its tests from.
const FAST_SUITES: [&str; 4] = ["unit", "contract", "integration", "ui"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Suite(String),
    Regression { all: bool },
    All,
}

impl Selection {
    /// `fast` and `all` only mean something for `regression`.
    pub fn parse(name: &str, fast: bool, all: bool) -> Result<Self> {
        if (fast || all) && name != "regression" {
            bail!("--fast and --all only go with `regression`");
        }
        Ok(match name {
            "regression" => Self::Regression { all },
            "all" => Self::All,
            suite if SUITES.contains(&suite) => Self::Suite(suite.to_owned()),
            other => bail!(
                "unknown test selection `{other}`: use regression, all or a suite ({})",
                SUITES.join(", ")
            ),
        })
    }

    fn picks(&self, test: &TestCase) -> bool {
        match self {
            Self::Suite(suite) => &test.tags.suite == suite,
            Self::Regression { all } => {
                test.tags.regression.is_some()
                    && (*all || FAST_SUITES.contains(&test.tags.suite.as_str()))
            }
            Self::All => true,
        }
    }
}

impl std::fmt::Display for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Suite(suite) => write!(f, "the {suite} suite"),
            Self::Regression { all: false } => f.write_str("regression --fast"),
            Self::Regression { all: true } => f.write_str("regression --all"),
            Self::All => f.write_str("all"),
        }
    }
}

pub fn select<'a>(index: &'a Index, selection: &Selection) -> Vec<&'a TestCase> {
    index.values().filter(|t| selection.picks(t)).collect()
}

pub fn run_selection(root: &Path, selection: &Selection, profile: &str, list: bool) -> Result<()> {
    let workspace = Workspace::load(root)?;
    let index = tags::build(root, &workspace)?;
    tags::write(root, &index)?;
    let chosen = select(&index, selection);
    if list {
        for test in &chosen {
            println!("{}", test.id);
        }
        return Ok(());
    }
    if chosen.is_empty() {
        println!("No tests in {selection} yet.");
        return Ok(());
    }

    let Batches { rust, qml, other } = Batches::split(chosen);
    // Run everything and report every failure, not just the first.
    let mut failed = Vec::new();
    if let Err(e) = run_rust(root, &workspace, &rust, profile) {
        failed.push(format!("{e:#}"));
    }
    if let Err(e) = qml::run_tests(root, &qml) {
        failed.push(format!("{e:#}"));
    }
    for test in other {
        if let Err(e) = run_other(root, test) {
            failed.push(format!("{e:#}"));
        }
    }
    if failed.is_empty() {
        return Ok(());
    }
    for failure in &failed {
        eprintln!("  {failure}");
    }
    bail!("{} part(s) of {selection} failed", failed.len());
}

/// The selected tests by how they run: Rust and QML tests in batches,
/// the others one by one.
#[derive(Debug, Default)]
struct Batches<'a> {
    rust: Vec<&'a TestCase>,
    qml: Vec<&'a TestCase>,
    other: Vec<&'a TestCase>,
}

impl<'a> Batches<'a> {
    fn split(tests: Vec<&'a TestCase>) -> Self {
        let mut batches = Self::default();
        for test in tests {
            match test.runner {
                Runner::Rust { .. } => batches.rust.push(test),
                Runner::Qml { .. } => batches.qml.push(test),
                Runner::Shell | Runner::Python { .. } => batches.other.push(test),
            }
        }
        batches
    }
}

/// One nextest run for all selected Rust tests, so the profile's JUnit
/// report covers them all. First checks that nextest finds exactly the
/// tests the index lists; a mismatch means the index is wrong.
fn run_rust(root: &Path, workspace: &Workspace, tests: &[&TestCase], profile: &str) -> Result<()> {
    if tests.is_empty() {
        return Ok(());
    }
    let packages: BTreeSet<String> = tests.iter().filter_map(|t| rust_package(t)).collect();
    let buildable: BTreeSet<String> = workspace
        .buildable_here(packages.iter().cloned().collect())
        .into_iter()
        .collect();
    for skipped in packages.difference(&buildable) {
        println!("Skipping {skipped}: it builds only on Linux.");
    }
    let runnable: Vec<&TestCase> = tests
        .iter()
        .copied()
        .filter(|t| rust_package(t).is_some_and(|p| buildable.contains(&p)))
        .collect();
    if runnable.is_empty() {
        return Ok(());
    }
    let packages = package_args(&buildable.into_iter().collect::<Vec<_>>());
    let filter = filterset(&runnable);

    let known = nextest_known(root, &packages, &filter)?;
    let (gated, missing): (Vec<&TestCase>, Vec<&TestCase>) = runnable
        .iter()
        .copied()
        .filter(|t| nextest_key(t).is_some_and(|k| !known.contains(&k)))
        .partition(|t| t.conditional);
    for test in &gated {
        println!("Not built here (behind a #[cfg]): {}", test.id);
    }
    if !missing.is_empty() {
        let ids: Vec<&str> = missing.iter().map(|t| t.id.as_str()).collect();
        bail!(
            "nextest doesn't know {} indexed test(s), so they would not run: {}",
            ids.len(),
            ids.join(", ")
        );
    }
    // nextest refuses a filter that matches nothing it built.
    let Some(filter) = filter_for_built(&runnable, &gated) else {
        return Ok(());
    };

    run(
        "Rust tests",
        nextest(root)
            .args(["--profile", profile, "--locked"])
            .args(&packages)
            .args(["-E", &filter]),
    )
}

/// `binary_id test_name` for every test nextest lists in these packages,
/// whether the filter matched it or not (an `#[ignore]`d test is listed but
/// never "matches").
fn nextest_known(root: &Path, packages: &[String], filter: &str) -> Result<BTreeSet<String>> {
    let output = cargo(root)
        .args(["nextest", "list", "--locked", "--message-format", "json"])
        .args(packages)
        .args(["-E", filter])
        .stderr(Stdio::inherit())
        .output()
        .context("running cargo nextest list")?;
    if !output.status.success() {
        bail!("cargo nextest list failed");
    }
    let list: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("reading nextest's test list")?;
    let mut known = BTreeSet::new();
    for (binary_id, suite) in list["rust-suites"].as_object().into_iter().flatten() {
        for name in suite["testcases"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(n, _)| n)
        {
            known.insert(format!("{binary_id} {name}"));
        }
    }
    Ok(known)
}

fn nextest_key(test: &TestCase) -> Option<String> {
    match &test.runner {
        Runner::Rust {
            binary_id, name, ..
        } => Some(format!("{binary_id} {name}")),
        _ => None,
    }
}

fn rust_package(test: &TestCase) -> Option<String> {
    match &test.runner {
        Runner::Rust { package, .. } => Some(package.clone()),
        _ => None,
    }
}

/// The filterset for the tests in `runnable` that this build has (those
/// not in `gated`), or `None` when there are none.
fn filter_for_built(runnable: &[&TestCase], gated: &[&TestCase]) -> Option<String> {
    let built: Vec<&TestCase> = runnable
        .iter()
        .copied()
        .filter(|t| !gated.contains(t))
        .collect();
    (!built.is_empty()).then(|| filterset(&built))
}

/// `(binary_id(=a) & (test(=x) | test(=y))) | …`: each test matched
/// exactly, grouped by binary.
pub fn filterset(tests: &[&TestCase]) -> String {
    let mut by_binary: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for test in tests {
        if let Runner::Rust {
            binary_id, name, ..
        } = &test.runner
        {
            by_binary.entry(binary_id).or_default().push(name);
        }
    }
    by_binary
        .into_iter()
        .map(|(binary_id, names)| {
            let names: Vec<String> = names.iter().map(|n| format!("test(={n})")).collect();
            format!("(binary_id(={binary_id}) & ({}))", names.join(" | "))
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn run_other(root: &Path, test: &TestCase) -> Result<()> {
    match &test.runner {
        Runner::Shell => {
            let result = run(&test.id, tool(root, "bash").arg(&test.file));
            let outcome = if result.is_ok() {
                Outcome::Passed
            } else {
                Outcome::Failed
            };
            // The test's own result matters more than recording it.
            if let Err(e) = results::record_script(root, &test.id, outcome) {
                eprintln!("warning: couldn't record the result of {}: {e:#}", test.id);
            }
            result
        }
        // Rust and QML tests run in batches, never here.
        _ => bail!("{}: no runner for this kind of test yet", test.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Tags;

    fn test(id: &str, suite: &str, regression: Option<&str>) -> TestCase {
        TestCase {
            id: id.into(),
            file: "f.rs".into(),
            line: 1,
            conditional: false,
            ignored: None,
            tags: Tags {
                covers: Vec::new(),
                suite: suite.into(),
                acceptance: regression.map(str::to_owned),
                regression: regression.map(str::to_owned),
                bug: None,
            },
            runner: Runner::Rust {
                package: "demo".into(),
                binary_id: "demo".into(),
                name: id.into(),
            },
        }
    }

    fn ids(index: &Index, selection: Selection) -> Vec<String> {
        select(index, &selection)
            .into_iter()
            .map(|t| t.id.clone())
            .collect()
    }

    // covers: QA-07
    // suite: unit
    #[test]
    fn regression_selector_picks_only_tagged_tests() {
        let index: Index = [
            test("a", "unit", Some("0.3")),
            test("b", "unit", None),
            test("c", "e2e", Some("0.3")),
            test("d", "integration", Some("0.4")),
        ]
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();

        assert_eq!(
            ids(&index, Selection::Regression { all: false }),
            ["a", "d"]
        );
        assert_eq!(
            ids(&index, Selection::Regression { all: true }),
            ["a", "c", "d"]
        );
        assert_eq!(ids(&index, Selection::Suite("unit".into())), ["a", "b"]);
        assert_eq!(ids(&index, Selection::All).len(), 4);
        assert!(Selection::parse("banana", false, false).is_err());
        assert!(Selection::parse("unit", false, true).is_err());
        assert_eq!(
            Selection::parse("regression", true, false).unwrap(),
            Selection::Regression { all: false }
        );
    }

    // covers: QA-07, QA-09
    // suite: unit
    #[test]
    fn tests_are_batched_by_runner_and_unbuilt_ones_left_out() {
        let rust = test("r", "unit", None);
        let mut qml = test("q", "ui", None);
        qml.runner = Runner::Qml {
            function: "test_q".into(),
        };
        let mut shell = test("s", "integration", None);
        shell.runner = Runner::Shell;
        let batches = Batches::split(vec![&rust, &qml, &shell]);
        assert_eq!(batches.rust, [&rust]);
        assert_eq!(batches.qml, [&qml]);
        assert_eq!(batches.other, [&shell]);

        let gated = test("g", "unit", None);
        assert_eq!(filter_for_built(&[&gated], &[&gated]), None);
        assert_eq!(
            filter_for_built(&[&rust, &gated], &[&gated]).as_deref(),
            Some("(binary_id(=demo) & (test(=r)))")
        );
    }

    // covers: QA-07
    // suite: unit
    #[test]
    fn filterset_matches_each_test_exactly() {
        let a = test("store::tests::appends", "unit", None);
        let b = test("paths::tests::falls_back", "unit", None);
        assert_eq!(
            filterset(&[&a, &b]),
            "(binary_id(=demo) & (test(=store::tests::appends) | test(=paths::tests::falls_back)))"
        );
    }
}
