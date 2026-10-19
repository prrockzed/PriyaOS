//! The test index: every test in the repository with the tags written
//! above it (sub-phase 0.4, grammar in docs/claude/architecture/09-testing.md
//! §3). `cargo xtask test` picks tests from it and `cargo xtask trace` checks
//! it against the feature catalog.
//!
//! ```text
//! // covers: CORE-01, CORE-03     (or: covers: infra)
//! // suite: unit
//! // acceptance: 0.5              (optional)
//! // regression: 0.5              (added at the sub-phase gate)
//! // bug: #42                     (optional)
//! #[test]
//! fn …
//! ```
//!
//! Rust and QML write `//`, shell and Python `#`. A shell test file is one
//! test; its tags sit in the header comment.

mod rust;
mod text;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::cmd::{TRACE_DIR, files_under, read, relative};
use crate::scope::Workspace;

/// Where tests live, relative to the repository root.
const ROOTS: [&str; 5] = ["crates", "modules", "ui", "scripts/tests", "xtask"];
/// Folders never scanned: build output and test data (a fixture repository
/// may hold tagged tests of its own).
const SKIPPED_DIRS: [&str; 4] = ["target", "fixtures", "node_modules", ".git"];

pub const SUITES: [&str; 11] = [
    "unit",
    "contract",
    "integration",
    "ui",
    "e2e",
    "perf",
    "chaos",
    "security",
    "eval",
    "live",
    "mutation",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestCase {
    pub id: String,
    /// Repository-relative, with forward slashes.
    pub file: String,
    pub line: usize,
    /// Behind a platform or feature `#[cfg]`, so not every build has it.
    pub conditional: bool,
    pub tags: Tags,
    pub runner: Runner,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Tags {
    /// Feature IDs; empty means `covers: infra`.
    pub covers: Vec<String>,
    pub suite: String,
    pub acceptance: Option<String>,
    pub regression: Option<String>,
    pub bug: Option<String>,
}

/// How to run one test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Runner {
    /// A nextest binary id (`priya-core`, `xtask::bin/xtask`,
    /// `priya-core::store`) and the test's name inside it.
    Rust {
        package: String,
        binary_id: String,
        name: String,
    },
    /// `bash <file>`.
    Shell,
    /// `qmltestrunner -input <file> -functions <function>`.
    Qml { function: String },
    /// `pytest <file> -k <function>`.
    Python { function: String },
}

/// The index, keyed by test id.
pub type Index = BTreeMap<String, TestCase>;

/// Scans the repository. Any malformed or missing tag fails the whole scan,
/// with one line per problem.
pub fn build(root: &Path, workspace: &Workspace) -> Result<Index> {
    let (index, problems) = scan(root, workspace)?;
    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("  {problem}");
        }
        bail!("{} test tag problem(s)", problems.len());
    }
    Ok(index)
}

/// Like `build`, but keeps every well-tagged test and ignores problems.
/// Used for the base branch, whose problems aren't the current change's.
pub fn build_lenient(root: &Path, workspace: &Workspace) -> Result<Index> {
    Ok(scan(root, workspace)?.0)
}

/// The index and the tag problems found on the way.
fn scan(root: &Path, workspace: &Workspace) -> Result<(Index, Vec<String>)> {
    let (rust_files, mut problems) = rust::module_tree(workspace);
    let mut found = Vec::new();

    // Files of real targets are indexed even under a `fixtures/` folder;
    // fixture repositories are never workspace targets.
    let mut proptest_reported = BTreeSet::new();
    for file in &rust_files {
        let rel = relative(root, &file.path);
        let source = read(&file.path)?;
        // A `proptest! { … }` block at the start of a line, not a mention.
        if source
            .lines()
            .any(|l| l.trim_start().starts_with("proptest!"))
            && proptest_reported.insert(rel.clone())
        {
            problems.push(format!(
                "{rel}: tests inside proptest! can't carry tags; write them as #[test] functions that call proptest's TestRunner"
            ));
        }
        found.extend(rust::tests_in(&source, &rel, file));
    }

    let in_targets = rust::paths(&rust_files);
    let roots: Vec<PathBuf> = ROOTS.iter().map(|r| root.join(r)).collect();
    for path in files_under(&roots, &SKIPPED_DIRS)? {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let rel = relative(root, &path);
        if name.ends_with(".rs") {
            if !in_targets.contains(path.as_path()) && rust::mentions_tests(&read(&path)?) {
                problems.push(format!(
                    "{rel}: has tests but isn't part of any lib, bin or test target, so they never run"
                ));
            }
        } else if is_text_test_file(&name) {
            found.extend(text::tests_in(&read(&path)?, &rel));
        }
    }

    let mut index = Index::new();
    for result in found {
        match result {
            Ok(test) => {
                if let Some(old) = index.get(&test.id) {
                    problems.push(format!(
                        "duplicate test id {}: {}:{} and {}:{}",
                        test.id, old.file, old.line, test.file, test.line
                    ));
                } else {
                    index.insert(test.id.clone(), test);
                }
            }
            Err(problem) => problems.push(problem),
        }
    }

    Ok((index, problems))
}

/// Writes `target/trace/tests.json` and returns its path.
pub fn write(root: &Path, index: &Index) -> Result<PathBuf> {
    let dir = root.join(TRACE_DIR);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join("tests.json");
    let json = serde_json::to_string_pretty(index).context("serialising the test index")?;
    fs::write(&path, json + "\n").with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// Parses the tag comments in `block` (the comment lines above a test,
/// prefix already removed). `at` names the test in error messages.
pub(crate) fn parse_tags(block: &[&str], at: &str) -> Result<Tags, String> {
    let mut tags = Tags::default();
    let mut seen: Vec<&str> = Vec::new();
    let mut covers_seen = false;
    for line in block {
        // Other comment lines, even ones with a colon, aren't tags.
        let Some((key, value)) = tag_line(line) else {
            continue;
        };
        if seen.contains(&key) {
            return Err(format!("{at}: `{key}:` appears twice"));
        }
        seen.push(key);
        if value.is_empty() {
            return Err(format!("{at}: `{key}:` has no value"));
        }
        match key {
            "covers" => {
                covers_seen = true;
                tags.covers = parse_covers(value).map_err(|e| format!("{at}: {e}"))?;
            }
            "suite" => {
                if !SUITES.contains(&value) {
                    return Err(format!(
                        "{at}: unknown suite `{value}` (one of {})",
                        SUITES.join(", ")
                    ));
                }
                tags.suite = value.to_owned();
            }
            "acceptance" | "regression" => {
                if !is_sub_phase(value) {
                    return Err(format!(
                        "{at}: `{key}:` needs a sub-phase like 0.5, got `{value}`"
                    ));
                }
                let value = Some(value.to_owned());
                if key == "acceptance" {
                    tags.acceptance = value;
                } else {
                    tags.regression = value;
                }
            }
            _ => tags.bug = Some(value.to_owned()),
        }
    }
    if !covers_seen {
        return Err(format!("{at}: missing `covers:` tag"));
    }
    if tags.suite.is_empty() {
        return Err(format!("{at}: missing `suite:` tag"));
    }
    Ok(tags)
}

const TAG_KEYS: [&str; 5] = ["covers", "suite", "acceptance", "regression", "bug"];

/// `(key, value)` when a comment line (marker already removed) is a tag.
pub fn tag_line(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    TAG_KEYS
        .iter()
        .find(|k| **k == key)
        .map(|k| (*k, value.trim()))
}

/// `infra`, or a comma-separated list of feature IDs such as `CORE-01`.
fn parse_covers(value: &str) -> Result<Vec<String>, String> {
    if value == "infra" {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(str::trim)
        .map(|id| {
            if is_feature_id(id) {
                Ok(id.to_owned())
            } else {
                Err(format!(
                    "`{id}` is not a feature ID like CORE-01 (or use `covers: infra`)"
                ))
            }
        })
        .collect()
}

/// Upper-case letters, a dash, two or more digits: `QA-05`, `CORE-01`.
pub fn is_feature_id(id: &str) -> bool {
    let Some((area, number)) = id.split_once('-') else {
        return false;
    };
    !area.is_empty()
        && area.bytes().all(|b| b.is_ascii_uppercase())
        && number.len() >= 2
        && number.bytes().all(|b| b.is_ascii_digit())
}

/// `0.5`, `2.11`.
pub fn is_sub_phase(value: &str) -> bool {
    value.split_once('.').is_some_and(|(phase, sub)| {
        [phase, sub]
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// QML, Python and shell test files, by their naming rules.
fn is_text_test_file(name: &str) -> bool {
    (name.starts_with("tst_") && name.ends_with(".qml"))
        || (name.starts_with("test_") && (name.ends_with(".sh") || name.ends_with(".py")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(lines: &[&str]) -> Result<Tags, String> {
        parse_tags(lines, "t")
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn tag_block_parses_and_rejects_malformed_tags() {
        let t = tags(&[
            "Explains the test: in plain words.",
            "covers: CORE-01, QA-05",
            "suite: integration",
            "acceptance: 0.4",
            "bug: #12",
        ])
        .unwrap();
        assert_eq!(t.covers, ["CORE-01", "QA-05"]);
        assert_eq!(t.suite, "integration");
        assert_eq!(t.acceptance.as_deref(), Some("0.4"));
        assert_eq!(t.regression, None);
        assert_eq!(t.bug.as_deref(), Some("#12"));
        assert!(
            tags(&["covers: infra", "suite: unit"])
                .unwrap()
                .covers
                .is_empty()
        );

        let err = |lines: &[&str]| tags(lines).unwrap_err();
        assert!(err(&["suite: unit"]).contains("missing `covers:`"));
        assert!(err(&["covers: infra"]).contains("missing `suite:`"));
        assert!(err(&["covers: core-1", "suite: unit"]).contains("not a feature ID"));
        assert!(err(&["covers: infra", "suite: banana"]).contains("unknown suite"));
        assert!(err(&["covers: infra", "suite: unit", "acceptance: soon"]).contains("sub-phase"));
        assert!(err(&["covers: infra", "covers: QA-01", "suite: unit"]).contains("twice"));
        assert!(err(&["covers:", "suite: unit"]).contains("no value"));
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn the_repository_index_builds_and_has_every_test_tagged() {
        let root = crate::cmd::workspace_root().unwrap();
        let index = build(&root, &Workspace::load(&root).unwrap()).unwrap();
        assert!(
            index.contains_key(
                "xtask::bin/xtask::tags::tests::the_repository_index_builds_and_has_every_test_tagged"
            ),
            "this test should index itself"
        );
        assert!(index.contains_key("scripts/tests/test_pre_commit.sh"));
    }
}
