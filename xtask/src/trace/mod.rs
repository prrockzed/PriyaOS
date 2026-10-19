//! `cargo xtask trace` (sub-phase 0.4, QA-05, QA-07): checks the test index
//! against the plan and writes the traceability matrix.
//!
//! It fails when:
//! 1. a test covers a feature ID that features.md doesn't have;
//! 2. a feature marked 🟨 or ✅ has no tagged test;
//! 3. a ✅ feature has no test that passed in this run (only when results
//!    are given);
//! 4. a finished sub-phase (every Done-when box ticked) has an
//!    `acceptance:` test without `regression:`;
//! 5. compared with the base branch, a `regression:` test was deleted or
//!    lost its tag, unless the PR cites an existing ADR or a new test with
//!    the same `covers` and `acceptance` replaces it (a rename).
//!
//! (A test without `suite:` already fails when the index is built.)

mod catalog;
#[cfg(test)]
mod end_to_end;
mod matrix;
pub mod results;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::cmd::{read, relative, run};
use crate::scope::Workspace;
use crate::tags::{self, Index, TestCase};
use catalog::{Feature, Status};
use results::{Outcome, Results};

pub const FEATURES: &str = "docs/claude/features.md";
const PHASES: &str = "docs/claude/phases";
const DECISIONS: &str = "docs/claude/architecture/decisions.md";

pub struct Options {
    /// JUnit reports from this run. With none, rule 3 is skipped.
    pub junit: Vec<PathBuf>,
    /// The branch to compare with for rule 5, e.g. `origin/main`.
    pub base: Option<String>,
    /// The pull request description, searched for ADR citations.
    pub pr_body: String,
    /// The shell tests' results from this run (scripts/tests/run.sh writes
    /// target/trace/script-results.json). Given explicitly, so a stale
    /// file from an earlier run never counts.
    pub script_results: Option<PathBuf>,
}

pub fn run_trace(root: &Path, options: &Options) -> Result<()> {
    let workspace = Workspace::load(root)?;
    let index = tags::build(root, &workspace)?;
    tags::write(root, &index)?;
    let features = catalog::features(&read(&root.join(FEATURES))?)?;
    let finished = finished_sub_phases(root)?;

    let mut results = Results::new();
    for path in &options.junit {
        results.extend(results::junit(&read(path)?)?);
    }
    if let Some(path) = &options.script_results {
        results.extend(results::read_file(path)?);
    }
    let has_results = !options.junit.is_empty() || options.script_results.is_some();

    let base = match &options.base {
        Some(base) => Some(base_index(root, base)?),
        None => None,
    };
    let cited = cited_adrs(&options.pr_body, &read(&root.join(DECISIONS))?);

    let inputs = Inputs {
        features: &features,
        finished: &finished,
        index: &index,
        results: has_results.then_some(&results),
        base: base.as_ref(),
        cited_adrs: &cited,
    };
    let Report { problems, notes } = check(&inputs);
    let written = matrix::write(root, &features, &index, &results)?;
    println!("Traceability matrix: {}", relative(root, &written));
    for note in &notes {
        println!("Note: {note}");
    }

    if problems.is_empty() {
        println!("Traceability: {} tests, all rules pass.", index.len());
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  {problem}");
    }
    bail!("{} traceability problem(s)", problems.len());
}

/// The quick check for the pre-commit hook: tags parse and name real
/// features.
pub fn check_tags(root: &Path) -> Result<()> {
    let index = tags::build(root, &Workspace::load(root)?)?;
    let features = catalog::features(&read(&root.join(FEATURES))?)?;
    let problems = unknown_features(&index, &features);
    if problems.is_empty() {
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  {problem}");
    }
    bail!("{} test(s) cover unknown features", problems.len());
}

struct Inputs<'a> {
    features: &'a BTreeMap<String, Feature>,
    finished: &'a BTreeSet<String>,
    index: &'a Index,
    /// `None` when this run has no test results.
    results: Option<&'a Results>,
    base: Option<&'a Index>,
    /// ADRs the PR cites that exist in decisions.md.
    cited_adrs: &'a [String],
}

/// What the rules found: problems fail the run; notes are shown, e.g. a
/// regression test whose removal a cited ADR allows.
#[derive(Debug, Default)]
struct Report {
    problems: Vec<String>,
    notes: Vec<String>,
}

fn check(inputs: &Inputs<'_>) -> Report {
    let mut problems = unknown_features(inputs.index, inputs.features);
    problems.extend(untested_features(inputs));
    problems.extend(unpromoted_acceptance_tests(inputs));
    let mut notes = Vec::new();
    for id in lost_regression_tests(inputs) {
        if inputs.cited_adrs.is_empty() {
            problems.push(format!(
                "regression test `{id}` was removed or lost its tag; keep it, or cite an ADR-NNN from decisions.md in the PR description"
            ));
        } else {
            let adrs = inputs.cited_adrs.join(", ");
            notes.push(format!("regression test `{id}` removed; allowed by {adrs}"));
        }
    }
    Report { problems, notes }
}

/// Rule 1.
fn unknown_features(index: &Index, features: &BTreeMap<String, Feature>) -> Vec<String> {
    index
        .values()
        .flat_map(|t| t.tags.covers.iter().map(move |id| (t, id)))
        .filter(|(_, id)| !features.contains_key(*id))
        .map(|(t, id)| format!("{}:{}: covers unknown feature {id}", t.file, t.line))
        .collect()
}

/// Rules 2 and 3.
fn untested_features(inputs: &Inputs<'_>) -> Vec<String> {
    let mut problems = Vec::new();
    for feature in inputs.features.values() {
        if !matches!(feature.status, Status::InProgress | Status::Done) {
            continue;
        }
        let tests: Vec<&TestCase> = tests_covering(inputs.index, &feature.id).collect();
        if tests.is_empty() {
            problems.push(format!(
                "{} is marked {} in features.md but no test covers it",
                feature.id,
                feature.status.symbol()
            ));
        } else if feature.status == Status::Done
            && let Some(results) = inputs.results
            && !tests
                .iter()
                .any(|t| results.get(&t.id) == Some(&Outcome::Passed))
        {
            problems.push(format!(
                "{} is marked ✅ but none of its {} test(s) passed in this run",
                feature.id,
                tests.len()
            ));
        }
    }
    problems
}

/// Rule 4.
fn unpromoted_acceptance_tests(inputs: &Inputs<'_>) -> Vec<String> {
    inputs
        .index
        .values()
        .filter(|t| t.tags.regression.is_none())
        .filter_map(|t| t.tags.acceptance.as_ref().map(|a| (t, a)))
        .filter(|(_, sub_phase)| inputs.finished.contains(*sub_phase))
        .map(|(t, sub_phase)| {
            format!(
                "{}:{}: {sub_phase} is finished, so `{}` needs `regression: {sub_phase}` (run `cargo xtask regression promote {sub_phase}`)",
                t.file, t.line, t.id
            )
        })
        .collect()
}

/// Rule 5: base-branch regression tests that are gone or untagged now and
/// weren't renamed. A rename is a new test carrying `regression:` with the
/// same `covers` and `acceptance`; each new test excuses one old test.
fn lost_regression_tests(inputs: &Inputs<'_>) -> Vec<String> {
    let Some(base) = inputs.base else {
        return Vec::new();
    };
    let mut renames: Vec<&TestCase> = inputs
        .index
        .values()
        .filter(|t| !base.contains_key(&t.id) && t.tags.regression.is_some())
        .collect();
    let mut lost = Vec::new();
    for old in base.values().filter(|t| t.tags.regression.is_some()) {
        let still_there = inputs
            .index
            .get(&old.id)
            .is_some_and(|now| now.tags.regression.is_some());
        if still_there {
            continue;
        }
        let rename = renames.iter().position(|t| {
            t.tags.covers == old.tags.covers && t.tags.acceptance == old.tags.acceptance
        });
        match rename {
            Some(i) => {
                renames.remove(i);
            }
            None => lost.push(old.id.clone()),
        }
    }
    lost
}

fn tests_covering<'a>(index: &'a Index, feature: &'a str) -> impl Iterator<Item = &'a TestCase> {
    index
        .values()
        .filter(move |t| t.tags.covers.iter().any(|c| c == feature))
}

fn finished_sub_phases(root: &Path) -> Result<BTreeSet<String>> {
    let mut finished = BTreeSet::new();
    let dir = root.join(PHASES);
    for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        let is_phase_file = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("phase-") && n.ends_with(".md"));
        if is_phase_file {
            finished.extend(catalog::finished_sub_phases(&read(&path)?));
        }
    }
    Ok(finished)
}

/// `ADR-NNN` ids in `body` that decisions.md defines (`## ADR-NNN — …`).
fn cited_adrs(body: &str, decisions: &str) -> Vec<String> {
    let mut cited: Vec<String> = Vec::new();
    for (i, _) in body.match_indices("ADR-") {
        let digits: String = body[i + 4..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if digits.len() == 3 {
            let id = format!("ADR-{digits}");
            if decisions.contains(&format!("## {id} ")) && !cited.contains(&id) {
                cited.push(id);
            }
        }
    }
    cited
}

/// The index as it is on `base`. The tree is exported with `git archive`
/// into a temporary folder, which touches nothing in `.git` and runs no
/// hooks. Tag problems on the base are ignored; they aren't this change's.
fn base_index(root: &Path, base: &str) -> Result<Index> {
    let temp = tempfile::tempdir().context("making a temporary folder")?;
    // Canonical, so paths match what cargo metadata reports (on macOS the
    // temp folder sits behind the /var -> /private/var link).
    let dir = temp
        .path()
        .canonicalize()
        .context("resolving the temporary folder")?;
    let archive = Command::new("git")
        .current_dir(root)
        .args(["archive", "--format=tar", base])
        .stdout(Stdio::piped())
        .spawn()
        .context("starting git archive")?;
    let Some(tar_input) = archive.stdout else {
        bail!("git archive has no output");
    };
    run(
        &format!("export {base} for comparison"),
        Command::new("tar")
            .arg("-x")
            .arg("-C")
            .arg(&dir)
            .stdin(tar_input),
    )?;
    let workspace =
        Workspace::load(&dir).with_context(|| format!("reading the workspace on {base}"))?;
    tags::build_lenient(&dir, &workspace).with_context(|| format!("indexing the tests on {base}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Runner, Tags};

    fn feature(id: &str, status: Status) -> (String, Feature) {
        (
            id.into(),
            Feature {
                id: id.into(),
                name: "x".into(),
                phase: "0".into(),
                status,
            },
        )
    }

    fn test(
        id: &str,
        covers: &[&str],
        acceptance: Option<&str>,
        regression: Option<&str>,
    ) -> (String, TestCase) {
        (
            id.into(),
            TestCase {
                id: id.into(),
                file: "f.rs".into(),
                line: 1,
                conditional: false,
                tags: Tags {
                    covers: covers.iter().map(|c| (*c).to_owned()).collect(),
                    suite: "unit".into(),
                    acceptance: acceptance.map(str::to_owned),
                    regression: regression.map(str::to_owned),
                    bug: None,
                },
                runner: Runner::Shell,
            },
        )
    }

    /// Runs the rules on a tiny plan: QA-05 in progress, CORE-01 done,
    /// CORE-02 planned, sub-phase 0.1 finished.
    fn check(
        index: &[(String, TestCase)],
        results: Option<&Results>,
        base: Option<&Index>,
        adrs: &[&str],
    ) -> Vec<String> {
        let features: BTreeMap<String, Feature> = [
            feature("QA-05", Status::InProgress),
            feature("CORE-01", Status::Done),
            feature("CORE-02", Status::Planned),
        ]
        .into_iter()
        .collect();
        let finished: BTreeSet<String> = ["0.1".to_owned()].into_iter().collect();
        let index: Index = index.iter().cloned().collect();
        let adrs: Vec<String> = adrs.iter().map(|a| (*a).to_owned()).collect();
        super::check(&Inputs {
            features: &features,
            finished: &finished,
            index: &index,
            results,
            base,
            cited_adrs: &adrs,
        })
        .problems
    }

    fn healthy() -> Vec<(String, TestCase)> {
        vec![
            test("qa", &["QA-05"], None, None),
            test("core", &["CORE-01"], Some("0.1"), Some("0.1")),
        ]
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn trace_fails_on_unknown_feature_id() {
        let mut index = healthy();
        index.push(test("typo", &["CORE-99"], None, None));
        let problems = check(&index, None, None, &[]);
        assert_eq!(problems, ["f.rs:1: covers unknown feature CORE-99"]);
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn trace_fails_when_in_progress_feature_has_no_tests() {
        let index = vec![test("core", &["CORE-01"], Some("0.1"), Some("0.1"))];
        let problems = check(&index, None, None, &[]);
        assert_eq!(
            problems,
            ["QA-05 is marked 🟨 in features.md but no test covers it"]
        );
        assert!(check(&healthy(), None, None, &[]).is_empty());
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn trace_fails_when_done_feature_has_no_passing_test() {
        let mut results = Results::new();
        results.insert("qa".into(), Outcome::Passed);
        results.insert("core".into(), Outcome::Failed);
        let problems = check(&healthy(), Some(&results), None, &[]);
        assert_eq!(
            problems,
            ["CORE-01 is marked ✅ but none of its 1 test(s) passed in this run"]
        );

        results.insert("core".into(), Outcome::Passed);
        assert!(check(&healthy(), Some(&results), None, &[]).is_empty());
        // Without results, rule 3 doesn't apply.
        assert!(check(&healthy(), None, None, &[]).is_empty());
    }

    // covers: QA-05, QA-07
    // suite: unit
    #[test]
    fn trace_fails_when_completed_subphase_has_unpromoted_acceptance_test() {
        let mut index = healthy();
        index.push(test("accept", &["QA-05"], Some("0.1"), None));
        index.push(test("later", &["QA-05"], Some("0.2"), None)); // 0.2 isn't finished
        let problems = check(&index, None, None, &[]);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("0.1 is finished, so `accept` needs `regression: 0.1`"));
    }

    // covers: QA-07
    // suite: unit
    #[test]
    fn trace_fails_when_regression_test_removed_without_adr() {
        let base: Index = healthy().into_iter().collect();
        let now = vec![test("qa", &["QA-05"], None, None)];
        let problems = check(&now, None, Some(&base), &[]);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(
            problems
                .iter()
                .any(|p| p.contains("regression test `core` was removed"))
        );

        // Citing an existing ADR allows the removal (CORE-01 then has no
        // test, which rule 2 still reports).
        let problems = check(&now, None, Some(&base), &["ADR-007"]);
        assert_eq!(
            problems,
            ["CORE-01 is marked ✅ in features.md but no test covers it"]
        );

        // Losing only the tag counts as removal too.
        let untagged = vec![
            test("qa", &["QA-05"], None, None),
            test("core", &["CORE-01"], Some("0.1"), None),
        ];
        assert!(
            check(&untagged, None, Some(&base), &[])
                .iter()
                .any(|p| p.contains("regression test `core` was removed"))
        );
    }

    // covers: QA-07
    // suite: unit
    #[test]
    fn a_cited_adr_turns_removals_into_notes() {
        let base: Index = healthy().into_iter().collect();
        let mut now = healthy();
        now.retain(|(id, _)| id != "core");
        now.push(test("core_new", &["CORE-01"], None, None));
        let features: BTreeMap<String, Feature> = [
            feature("QA-05", Status::InProgress),
            feature("CORE-01", Status::Done),
        ]
        .into_iter()
        .collect();
        let index: Index = now.into_iter().collect();
        let report = super::check(&Inputs {
            features: &features,
            finished: &BTreeSet::new(),
            index: &index,
            results: None,
            base: Some(&base),
            cited_adrs: &["ADR-007".to_owned()],
        });
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(
            report.notes,
            ["regression test `core` removed; allowed by ADR-007"]
        );
    }

    // covers: QA-07
    // suite: unit
    #[test]
    fn trace_allows_regression_test_rename() {
        let base: Index = healthy().into_iter().collect();
        let renamed = vec![
            test("qa", &["QA-05"], None, None),
            test("core_renamed", &["CORE-01"], Some("0.1"), Some("0.1")),
        ];
        assert!(check(&renamed, None, Some(&base), &[]).is_empty());

        // The new test must itself be a regression test…
        let untagged = vec![
            test("qa", &["QA-05"], None, None),
            test("core_renamed", &["CORE-01"], Some("0.1"), None),
        ];
        assert!(!check(&untagged, None, Some(&base), &[]).is_empty());

        // …and one new test excuses only one removed test.
        let mut base_two = healthy();
        base_two.push(test("core_too", &["CORE-01"], Some("0.1"), Some("0.1")));
        let base_two: Index = base_two.into_iter().collect();
        let problems = check(&renamed, None, Some(&base_two), &[]);
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    // covers: QA-07
    // suite: unit
    #[test]
    fn only_existing_adrs_count_as_cited() {
        let decisions = "## ADR-007 — Something\n\n## ADR-022 — Licence\n";
        assert_eq!(
            cited_adrs("Removes it, see ADR-022 and ADR-999 and ADR-22.", decisions),
            ["ADR-022"]
        );
    }
}
