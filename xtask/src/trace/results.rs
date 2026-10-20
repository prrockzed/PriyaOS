//! What the last run said: nextest's JUnit report for Rust tests,
//! `target/trace/script-results.json` for shell tests (written by
//! scripts/tests/run.sh and `cargo xtask test`) and
//! `target/trace/ui-results.json` for QML tests (`cargo xtask test`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};

pub const SCRIPT_RESULTS: &str = "target/trace/script-results.json";
pub const UI_RESULTS: &str = "target/trace/ui-results.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Passed,
    Failed,
    Skipped,
}

/// Test id → outcome. Rust ids are `<classname>::<name>`, which is exactly
/// `<binary id>::<test name>`, the index's id.
pub type Results = BTreeMap<String, Outcome>;

pub fn junit(xml: &str) -> Result<Results> {
    let mut results = Results::new();
    let mut reader = quick_xml::Reader::from_str(xml);
    // The open <testcase>: its id and the outcome its children have set.
    let mut open: Option<(String, Outcome)> = None;
    loop {
        match reader.read_event().context("reading the JUnit report")? {
            // <testcase …/> has no children: it passed.
            Event::Empty(e) if e.name().as_ref() == "testcase" => {
                results.insert(testcase_id(&e)?, Outcome::Passed);
            }
            Event::Start(e) if e.name().as_ref() == "testcase" => {
                open = Some((testcase_id(&e)?, Outcome::Passed));
            }
            Event::Start(e) | Event::Empty(e) => {
                if let Some((_, outcome)) = open.as_mut() {
                    match e.name().as_ref() {
                        "failure" | "error" => *outcome = Outcome::Failed,
                        "skipped" => *outcome = Outcome::Skipped,
                        _ => {} // output, flaky retries
                    }
                }
            }
            Event::End(e) if e.name().as_ref() == "testcase" => {
                if let Some((id, outcome)) = open.take() {
                    results.insert(id, outcome);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(results)
}

/// `<classname>::<name>` of a `<testcase>` element.
fn testcase_id(element: &quick_xml::events::BytesStart<'_>) -> Result<String> {
    let mut name = String::new();
    let mut class = String::new();
    for attr in element.attributes() {
        let attr = attr.context("a JUnit attribute")?;
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Explicit1_0)
            .context("a JUnit attribute value")?;
        match attr.key.as_ref() {
            "name" => name = value.into_owned(),
            "classname" => class = value.into_owned(),
            _ => {}
        }
    }
    Ok(format!("{class}::{name}"))
}

/// Records one shell test's outcome, keeping the others.
pub fn record_script(root: &Path, id: &str, outcome: Outcome) -> Result<()> {
    record(
        &root.join(SCRIPT_RESULTS),
        &Results::from([(id.to_owned(), outcome)]),
    )
}

/// Merges `new` into the results file at `path`, keeping other tests'
/// entries.
pub fn record(path: &Path, new: &Results) -> Result<()> {
    let mut results = read_file(path)?;
    results.extend(new.iter().map(|(id, outcome)| (id.clone(), *outcome)));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let json = serde_json::to_string_pretty(&results).context("serialising results")?;
    fs::write(path, json + "\n").with_context(|| format!("writing {}", path.display()))
}

/// A results file written by `record` or scripts/tests/run.sh; nothing
/// when no such tests have run.
pub fn read_file(path: &Path) -> Result<Results> {
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Results::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-05
    // suite: unit
    #[test]
    fn junit_outcomes_are_read_per_test() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run">
  <testsuite name="demo">
    <testcase name="a::passes" classname="demo" time="0.01"/>
    <testcase name="a::fails" classname="demo"><failure type="test failure">boom</failure><system-out>x</system-out></testcase>
    <testcase name="a::skipped" classname="demo"><skipped/></testcase>
    <testcase name="works" classname="demo::bin/demo" time="0.01"></testcase>
  </testsuite>
</testsuites>"#;
        let results = junit(xml).unwrap();
        assert_eq!(results["demo::a::passes"], Outcome::Passed);
        assert_eq!(results["demo::a::fails"], Outcome::Failed);
        assert_eq!(results["demo::a::skipped"], Outcome::Skipped);
        assert_eq!(results["demo::bin/demo::works"], Outcome::Passed);
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn script_results_are_merged_per_test() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(SCRIPT_RESULTS);
        assert!(read_file(&file).unwrap().is_empty());
        record_script(dir.path(), "scripts/tests/test_a.sh", Outcome::Passed).unwrap();
        record_script(dir.path(), "scripts/tests/test_b.sh", Outcome::Failed).unwrap();
        let results = read_file(&file).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results["scripts/tests/test_b.sh"], Outcome::Failed);
    }
}
