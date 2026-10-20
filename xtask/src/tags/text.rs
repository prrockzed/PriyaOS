//! Tests in QML, Python and shell, found by their naming rules:
//!
//! - QML (`tst_*.qml`): each `function test_*(` with `//` tags above it,
//!   except `test_*_data`, which supplies a data-driven test's rows.
//! - Python (`test_*.py`): each `def test_*(` with `#` tags above it.
//! - Shell (`test_*.sh`): the whole file is one test, tagged in the header
//!   comment under the `#!` line.

use super::{Runner, TestCase, parse_tags};

pub fn tests_in(source: &str, file: &str) -> Vec<Result<TestCase, String>> {
    let lines: Vec<&str> = source.lines().collect();
    if file.ends_with(".sh") {
        vec![shell_test(&lines, file)]
    } else if file.ends_with(".qml") {
        functions(&lines, file, "function ", "//", |function| Runner::Qml {
            function,
        })
    } else {
        functions(&lines, file, "def ", "#", |function| Runner::Python {
            function,
        })
    }
}

fn shell_test(lines: &[&str], file: &str) -> Result<TestCase, String> {
    let block: Vec<&str> = lines
        .iter()
        .skip_while(|l| l.starts_with("#!"))
        .map_while(|l| l.trim_start().strip_prefix('#'))
        .collect();
    Ok(TestCase {
        id: file.to_owned(),
        file: file.to_owned(),
        line: 1,
        conditional: false,
        ignored: None,
        tags: parse_tags(&block, file)?,
        runner: Runner::Shell,
    })
}

/// Functions named `test_*` introduced by `keyword`, tagged with `comment`.
fn functions(
    lines: &[&str],
    file: &str,
    keyword: &str,
    comment: &str,
    runner: impl Fn(String) -> Runner,
) -> Vec<Result<TestCase, String>> {
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let line = line.trim_start();
        let line = line.strip_prefix("async ").unwrap_or(line);
        let Some(rest) = line.strip_prefix(keyword) else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.starts_with("test_") || (keyword == "function " && name.ends_with("_data")) {
            continue;
        }
        let at = format!("{file}:{}", i + 1);
        let block: Vec<&str> = {
            // Python decorators may sit between the tags and the `def`.
            let mut block: Vec<&str> = lines[..i]
                .iter()
                .rev()
                .map(|l| l.trim_start())
                .filter(|l| !l.starts_with('@'))
                .map_while(|l| l.strip_prefix(comment))
                .collect();
            block.reverse();
            block
        };
        found.push(parse_tags(&block, &at).map(|tags| TestCase {
            id: format!("{file}::{name}"),
            file: file.to_owned(),
            line: i + 1,
            conditional: false,
            ignored: None,
            tags,
            runner: runner(name),
        }));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(found: Vec<Result<TestCase, String>>) -> Vec<TestCase> {
        found.into_iter().map(|r| r.unwrap()).collect()
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn tags_parse_rust_qml_shell_python() {
        // Rust has its own test in tags/rust.rs; this covers the other three.
        let qml = "TestCase {\n    // covers: DS-01\n    // suite: ui\n    function test_switches_mode() {}\n    function helper() {}\n    function test_switches_mode_data() {}\n}\n";
        let qml = ok(tests_in(qml, "ui/tests/tst_theme.qml"));
        assert_eq!(qml.len(), 1);
        assert_eq!(qml[0].id, "ui/tests/tst_theme.qml::test_switches_mode");
        assert_eq!(qml[0].line, 4);
        assert_eq!(
            qml[0].runner,
            Runner::Qml {
                function: "test_switches_mode".into()
            }
        );

        let python = "# covers: AI-01, AI-02\n# suite: eval\ndef test_router_picks_cheapest():\n    pass\n\n# covers: AI-03\n# suite: eval\n@pytest.mark.slow\nasync def test_streams():\n    pass\n";
        let python = ok(tests_in(python, "crates/priya-ai/evals/test_router.py"));
        assert_eq!(python[0].tags.covers, ["AI-01", "AI-02"]);
        assert_eq!(python[0].tags.suite, "eval");
        assert_eq!(
            python[1].id,
            "crates/priya-ai/evals/test_router.py::test_streams"
        );
        assert_eq!(python[1].tags.covers, ["AI-03"]);

        let shell = "#!/usr/bin/env bash\n# covers: QA-03\n# suite: integration\n# acceptance: 0.3\n#\n# what it checks: details\n\nset -u\n";
        let shell = ok(tests_in(shell, "scripts/tests/test_hooks.sh"));
        assert_eq!(shell[0].id, "scripts/tests/test_hooks.sh");
        assert_eq!(shell[0].tags.acceptance.as_deref(), Some("0.3"));
        assert_eq!(shell[0].runner, Runner::Shell);

        let untagged = tests_in("#!/bin/sh\necho hi\n", "scripts/tests/test_x.sh");
        assert!(
            untagged[0]
                .as_ref()
                .unwrap_err()
                .contains("missing `covers:`")
        );
    }
}
