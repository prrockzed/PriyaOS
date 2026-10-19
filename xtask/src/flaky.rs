//! The flaky-test quarantine (sub-phase 0.4, QA-11).
//!
//! A test that fails now and then is quarantined, not retried: it gets
//! `#[ignore = "flaky: #<issue>"]` and an entry in `tests/quarantine.toml`
//! naming its tracking issue and the day it went in. The quarantine is kept
//! small and short so flaky tests get fixed:
//!
//! - `cargo xtask flaky check` fails when an attribute has no entry or an
//!   entry no attribute, when there are more than 5 entries, or when an
//!   entry is older than 7 days (it warns from day 5). It prints the list,
//!   also into the GitHub job summary.
//! - `cargo xtask flaky hunt` (nightly) runs each quarantined test many
//!   times and the unit and contract suites a few times, and writes the
//!   pass rates for the nightly workflow to post on the tracking issues.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use priya_core::clock::Clock;
use serde::Serialize;
use time::{Date, macros::format_description};

use crate::cmd::{TRACE_DIR, nextest, read};
use crate::scope::Workspace;
use crate::tags::{self, Index, Runner};
use crate::test_cmd::filterset;

pub const QUARANTINE: &str = "tests/quarantine.toml";
const MAX_ENTRIES: usize = 5;
const WARN_AFTER_DAYS: i64 = 5;
const FAIL_AFTER_DAYS: i64 = 7;
const FLAKY_PREFIX: &str = "flaky: #";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub test: String,
    pub issue: u64,
    pub since: Date,
}

/// What the check found.
#[derive(Debug, Default, PartialEq, Eq)]
struct Findings {
    problems: Vec<String>,
    warnings: Vec<String>,
}

pub fn check(root: &Path, clock: &dyn Clock) -> Result<()> {
    let entries = entries(&read(&root.join(QUARANTINE))?)?;
    let index = tags::build(root, &Workspace::load(root)?)?;
    let today = clock.now().date();
    let findings = findings(&entries, &index, today);

    let mut summary = summary(&entries, today);
    for line in findings
        .warnings
        .iter()
        .map(|w| format!("- ⚠️ {w}"))
        .chain(findings.problems.iter().map(|p| format!("- ❌ {p}")))
    {
        summary.push_str(&line);
        summary.push('\n');
    }
    println!("{summary}");
    // For the PR report.
    let dir = root.join(TRACE_DIR);
    std::fs::create_dir_all(&dir).context("creating target/trace")?;
    std::fs::write(dir.join("quarantine.md"), &summary).context("writing quarantine.md")?;
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        // Best effort: the check's verdict doesn't depend on the summary.
        let _ = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, summary.as_bytes()));
    }
    for warning in &findings.warnings {
        eprintln!("  warning: {warning}");
    }
    if findings.problems.is_empty() {
        return Ok(());
    }
    for problem in &findings.problems {
        eprintln!("  {problem}");
    }
    bail!("{} quarantine problem(s)", findings.problems.len());
}

/// Parses `[[test]]` tables: `test = "<id>"`, `issue = <n>`,
/// `since = "YYYY-MM-DD"`.
fn entries(toml: &str) -> Result<Vec<Entry>> {
    let doc: toml_edit::DocumentMut = toml.parse().context("reading tests/quarantine.toml")?;
    let Some(tables) = doc.get("test") else {
        return Ok(Vec::new());
    };
    let tables = tables
        .as_array_of_tables()
        .context("quarantine.toml: `test` must be a list of [[test]] tables")?;
    let date_format = format_description!("[year]-[month]-[day]");
    tables
        .iter()
        .enumerate()
        .map(|(i, table)| {
            let n = i + 1;
            let test = table
                .get("test")
                .and_then(|v| v.as_str())
                .with_context(|| format!("quarantine entry {n}: `test` must be a test id"))?;
            let issue = table
                .get("issue")
                .and_then(|v| v.as_integer())
                .and_then(|v| u64::try_from(v).ok())
                .with_context(|| {
                    format!("quarantine entry {n}: `issue` must be an issue number")
                })?;
            let since = table
                .get("since")
                .and_then(|v| v.as_str())
                .with_context(|| format!("quarantine entry {n}: `since` must be a date"))?;
            let since = Date::parse(since, date_format).with_context(|| {
                format!("quarantine entry {n}: `since` must look like 2026-10-20")
            })?;
            Ok(Entry {
                test: test.to_owned(),
                issue,
                since,
            })
        })
        .collect()
}

fn findings(entries: &[Entry], index: &Index, today: Date) -> Findings {
    let mut found = Findings::default();
    if entries.len() > MAX_ENTRIES {
        found.problems.push(format!(
            "{} tests are quarantined; the limit is {MAX_ENTRIES}. Fix some before adding more.",
            entries.len()
        ));
    }

    // Attributes: test id → issue number.
    let marked: BTreeMap<&str, Option<u64>> = index
        .values()
        .filter_map(|t| {
            // Any reason starting "flaky" is a quarantine; only the exact
            // `flaky: #<n>` form names its issue.
            let reason = t.ignored.as_deref()?.trim();
            if !reason.to_ascii_lowercase().starts_with("flaky") {
                return None;
            }
            let issue = reason
                .strip_prefix(FLAKY_PREFIX)
                .and_then(|n| n.trim().parse().ok());
            Some((t.id.as_str(), issue))
        })
        .collect();

    for (test, issue) in &marked {
        match (entries.iter().find(|e| e.test == *test), issue) {
            (_, None) => found.problems.push(format!(
                "{test}: write the ignore reason as `flaky: #<issue number>`"
            )),
            (None, _) => found.problems.push(format!(
                "{test} is marked flaky but has no entry in {QUARANTINE}"
            )),
            (Some(entry), Some(issue)) if entry.issue != *issue => found.problems.push(format!(
                "{test}: the attribute says #{issue} but {QUARANTINE} says #{}",
                entry.issue
            )),
            _ => {}
        }
    }

    let mut seen = std::collections::BTreeSet::new();
    for entry in entries {
        if !seen.insert(entry.test.as_str()) {
            found.problems.push(format!(
                "{} appears more than once in {QUARANTINE}",
                entry.test
            ));
        }
        if !marked.contains_key(entry.test.as_str()) {
            found.problems.push(format!(
                "{} is in {QUARANTINE} but not marked `#[ignore = \"flaky: #{}\"]` (or no longer exists)",
                entry.test, entry.issue
            ));
        }
        let days = (today - entry.since).whole_days();
        // One day of slack: `since` is a local date and `today` is UTC.
        if days < -1 {
            found.problems.push(format!(
                "{}: `since` ({}) is in the future",
                entry.test, entry.since
            ));
        } else if days > FAIL_AFTER_DAYS {
            found.problems.push(format!(
                "{} has been quarantined for {days} days (limit {FAIL_AFTER_DAYS}); fix it or remove it (#{})",
                entry.test, entry.issue
            ));
        } else if days >= WARN_AFTER_DAYS {
            found.warnings.push(format!(
                "{} has been quarantined for {days} days; it fails the build after {FAIL_AFTER_DAYS}",
                entry.test
            ));
        }
    }
    found
}

/// The quarantine list as Markdown, for the console and the job summary.
fn summary(entries: &[Entry], today: Date) -> String {
    let mut md = String::from("### Quarantined tests\n\n");
    if entries.is_empty() {
        md.push_str("None.\n");
        return md;
    }
    md.push_str("| Test | Issue | Days |\n|---|---|---|\n");
    for entry in entries {
        let days = (today - entry.since).whole_days().max(0);
        let _ = writeln!(md, "| `{}` | #{} | {days} |", entry.test, entry.issue);
    }
    md
}

/// One quarantined test's nightly pass rate.
#[derive(Debug, Serialize)]
struct HuntResult {
    test: String,
    issue: u64,
    runs: u32,
    passed: u32,
}

/// Runs each quarantined Rust test `runs` times and the unit and contract
/// suites `suite_runs` times (never retrying), then writes
/// `target/trace/flaky-hunt.json` for the nightly workflow. Fails when a
/// suite run fails, since that means a new flaky test.
pub fn hunt(root: &Path, runs: u32, suite_runs: u32) -> Result<()> {
    let entries = entries(&read(&root.join(QUARANTINE))?)?;
    let index = tags::build(root, &Workspace::load(root)?)?;

    let mut results = Vec::new();
    for entry in &entries {
        let Some(test) = index.get(&entry.test) else {
            continue; // `flaky check` reports it
        };
        // QML quarantine arrives with the QML runner (0.11).
        let Runner::Rust { package, .. } = &test.runner else {
            continue;
        };
        let filter = filterset(&[test]);
        let mut passed = 0;
        for run in 1..=runs {
            // stdout is the test's own chatter; errors still reach stderr.
            let status = nextest(root)
                .args(["--profile", "nightly", "--locked", "-p", package])
                .args(["--run-ignored", "only", "-E", &filter])
                .stdout(Stdio::null())
                .status()
                .context("running a quarantined test")?;
            // nextest exits 100 when tests fail; anything else (no test
            // matched, a build error) says nothing about flakiness.
            match status.code() {
                Some(0) => passed += 1,
                Some(100) => {}
                _ => bail!(
                    "{}: run {run} didn't run the test ({status}); is the id still right?",
                    entry.test
                ),
            }
        }
        println!("{}: {passed}/{runs} passed", entry.test);
        results.push(HuntResult {
            test: entry.test.clone(),
            issue: entry.issue,
            runs,
            passed,
        });
    }

    let path = root.join(TRACE_DIR).join("flaky-hunt.json");
    std::fs::create_dir_all(root.join(TRACE_DIR)).context("creating target/trace")?;
    std::fs::write(&path, serde_json::to_string_pretty(&results)? + "\n")
        .with_context(|| format!("writing {}", path.display()))?;

    // Every suite run happens; failures are reported together at the end.
    let mut failures = Vec::new();
    for suite in ["unit", "contract"] {
        for run in 1..=suite_runs {
            let selection = crate::test_cmd::Selection::Suite(suite.to_owned());
            if let Err(e) = crate::test_cmd::run_selection(root, &selection, "nightly", false) {
                failures.push(format!("{suite} run {run} of {suite_runs}: {e:#}"));
            }
        }
    }
    if failures.is_empty() {
        return Ok(());
    }
    for failure in &failures {
        eprintln!("  {failure}");
    }
    bail!("{} suite run(s) failed: a new flaky test?", failures.len());
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use priya_testkit::FakeClock;
    use time::macros::date;

    use super::*;
    use crate::tags::{Tags, TestCase};

    fn test(id: &str, ignored: Option<&str>) -> (String, TestCase) {
        (
            id.into(),
            TestCase {
                id: id.into(),
                file: "f.rs".into(),
                line: 1,
                conditional: false,
                ignored: ignored.map(str::to_owned),
                tags: Tags {
                    covers: Vec::new(),
                    suite: "unit".into(),
                    ..Tags::default()
                },
                runner: Runner::Shell,
            },
        )
    }

    fn entry(test: &str, issue: u64, since: Date) -> Entry {
        Entry {
            test: test.into(),
            issue,
            since,
        }
    }

    const DAY: Date = date!(2026 - 10 - 20);

    // covers: QA-11
    // suite: unit
    #[test]
    fn quarantine_entries_are_parsed() {
        let toml = "# comment\n[[test]]\ntest = \"demo::a\"\nissue = 42\nsince = \"2026-10-20\"\n";
        assert_eq!(entries(toml).unwrap(), [entry("demo::a", 42, DAY)]);
        assert!(entries("").unwrap().is_empty());
        assert!(entries("[[test]]\ntest = \"x\"\nissue = 1\nsince = \"20/10/2026\"\n").is_err());
    }

    // covers: QA-11
    // suite: unit
    // acceptance: 0.4
    #[test]
    fn flaky_check_fails_over_five_entries() {
        let index: Index = (0..6)
            .map(|i| test(&format!("t{i}"), Some(&format!("flaky: #{i}"))))
            .collect();
        let entries: Vec<Entry> = (0..6).map(|i| entry(&format!("t{i}"), i, DAY)).collect();
        let found = findings(&entries, &index, DAY);
        assert_eq!(found.problems.len(), 1, "{:?}", found.problems);
        assert!(found.problems[0].contains("6 tests are quarantined; the limit is 5"));
        assert!(
            findings(&entries[..5], &index.into_iter().take(5).collect(), DAY)
                .problems
                .is_empty()
        );
    }

    // covers: QA-11
    // suite: unit
    // acceptance: 0.4
    #[test]
    fn flaky_check_fails_on_attribute_without_entry() {
        let index: Index = [
            test("marked", Some("flaky: #7")),
            test("wrong_issue", Some("flaky: #8")),
            test("bad_reason", Some("flaky: soon")),
            test("ignored_for_other_reasons", Some("needs a GPU")),
        ]
        .into_iter()
        .collect();
        let entries = [entry("wrong_issue", 9, DAY), entry("gone", 3, DAY)];
        let problems = findings(&entries, &index, DAY).problems;
        assert_eq!(
            problems,
            [
                "bad_reason: write the ignore reason as `flaky: #<issue number>`",
                "marked is marked flaky but has no entry in tests/quarantine.toml",
                "wrong_issue: the attribute says #8 but tests/quarantine.toml says #9",
                "gone is in tests/quarantine.toml but not marked `#[ignore = \"flaky: #3\"]` (or no longer exists)",
            ]
        );
    }

    // covers: QA-11
    // suite: unit
    // acceptance: 0.4
    #[test]
    fn flaky_check_fails_after_seven_days() {
        let clock = FakeClock::new();
        let since = clock.now().date();
        let index: Index = [test("slow", Some("flaky: #5"))].into_iter().collect();
        let entries = [entry("slow", 5, since)];
        let at = |clock: &FakeClock| findings(&entries, &index, clock.now().date());

        assert_eq!(at(&clock), Findings::default());
        clock.advance(Duration::from_secs(5 * 86_400));
        let day_five = at(&clock);
        assert!(day_five.problems.is_empty());
        assert_eq!(day_five.warnings.len(), 1, "warns from day 5");
        clock.advance(Duration::from_secs(2 * 86_400));
        assert!(at(&clock).problems.is_empty(), "day 7 is still allowed");
        clock.advance(Duration::from_secs(86_400));
        let day_eight = at(&clock);
        assert_eq!(day_eight.problems.len(), 1);
        assert!(day_eight.problems[0].contains("quarantined for 8 days (limit 7)"));
    }

    // covers: QA-11
    // suite: unit
    #[test]
    fn future_dates_and_duplicates_are_rejected() {
        let index: Index = [test("t", Some("flaky: #1"))].into_iter().collect();
        // Tomorrow is allowed (time zones); later isn't.
        let tomorrow = [entry("t", 1, date!(2026 - 10 - 21))];
        assert!(findings(&tomorrow, &index, DAY).problems.is_empty());
        let later = [entry("t", 1, date!(2099 - 01 - 01))];
        assert_eq!(
            findings(&later, &index, DAY).problems,
            ["t: `since` (2099-01-01) is in the future"]
        );
        let twice = [entry("t", 1, DAY), entry("t", 1, DAY)];
        assert_eq!(
            findings(&twice, &index, DAY).problems,
            ["t appears more than once in tests/quarantine.toml"]
        );
        assert!(summary(&tomorrow, DAY).contains("| `t` | #1 | 0 |"));
    }

    // covers: QA-11
    // suite: unit
    #[test]
    fn summary_lists_the_quarantine() {
        assert_eq!(summary(&[], DAY), "### Quarantined tests\n\nNone.\n");
        let md = summary(&[entry("demo::a", 42, date!(2026 - 10 - 18))], DAY);
        assert!(md.contains("| `demo::a` | #42 | 2 |"), "{md}");
    }
}
