//! `cargo xtask regression promote <sub-phase>`: at a sub-phase's gate, adds
//! `regression: <sub-phase>` under every `acceptance: <sub-phase>` tag, which
//! puts those tests in the permanent regression suite (QA-07). Running it
//! again changes nothing.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::scope::Workspace;
use crate::tags::{self, TestCase, is_sub_phase, tag_line};

pub fn promote(root: &Path, sub_phase: &str) -> Result<()> {
    if !is_sub_phase(sub_phase) {
        bail!("`{sub_phase}` is not a sub-phase like 0.4");
    }
    let index = tags::build(root, &Workspace::load(root)?)?;
    let mut by_file: BTreeMap<&str, Vec<&TestCase>> = BTreeMap::new();
    for test in index.values() {
        if test.tags.acceptance.as_deref() == Some(sub_phase) && test.tags.regression.is_none() {
            by_file.entry(test.file.as_str()).or_default().push(test);
        }
    }

    let mut promoted = 0;
    for (file, tests) in by_file {
        let path = root.join(file);
        let source = fs::read_to_string(&path).with_context(|| format!("reading {file}"))?;
        let lines: Vec<usize> = tests.iter().map(|t| t.line).collect();
        let updated = add_regression_tags(&source, &lines, sub_phase)
            .with_context(|| format!("promoting tests in {file}"))?;
        fs::write(&path, updated).with_context(|| format!("writing {file}"))?;
        promoted += tests.len();
    }
    println!("Promoted {promoted} test(s) to the {sub_phase} regression suite.");
    Ok(())
}

/// For each test starting at one of `test_lines` (1-based), finds its
/// `acceptance: <sub_phase>` tag and inserts the matching `regression:` line
/// right below it, with the same indent and comment marker. Line endings
/// (LF or CRLF) are kept.
fn add_regression_tags(source: &str, test_lines: &[usize], sub_phase: &str) -> Result<String> {
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let mut inserts: Vec<(usize, String)> = Vec::new();
    for &test_line in test_lines {
        let Some(at) = acceptance_line(&lines, test_line, sub_phase) else {
            bail!("no `acceptance: {sub_phase}` tag for the test at line {test_line}");
        };
        inserts.push((at + 1, lines[at].replacen("acceptance", "regression", 1)));
    }
    // Insert from the bottom so earlier line numbers stay valid.
    inserts.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    inserts.dedup_by_key(|(at, _)| *at);
    for (at, line) in inserts {
        lines.insert(at, line);
    }
    let mut out = lines.join(newline);
    if source.ends_with('\n') {
        out.push_str(newline);
    }
    Ok(out)
}

/// The 0-based line of the test's `acceptance: <sub_phase>` tag. A shell
/// test (line 1) is tagged in its header comment, read from the top; any
/// other test in the comment lines just above it.
fn acceptance_line(lines: &[String], test_line: usize, sub_phase: &str) -> Option<usize> {
    let is_tag = |i: &usize| tag_value(&lines[*i], "acceptance") == Some(sub_phase);
    if test_line == 1 {
        (0..lines.len())
            .skip_while(|&i| lines[i].starts_with("#!"))
            .take_while(|&i| lines[i].trim_start().starts_with('#'))
            .find(is_tag)
    } else {
        (0..test_line.saturating_sub(1).min(lines.len()))
            .rev()
            .find(is_tag)
    }
}

/// The value of a `// key: value` or `# key: value` tag line.
fn tag_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let line = line.trim_start();
    let comment = line.strip_prefix("//").or_else(|| line.strip_prefix('#'))?;
    tag_line(comment).filter(|(k, _)| *k == key).map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-07
    // suite: unit
    #[test]
    fn promote_adds_regression_tag_idempotently() {
        let rust = "\
mod tests {
    // covers: CORE-01
    // suite: unit
    // acceptance: 0.5
    #[test]
    fn appends() {}

    // covers: CORE-01
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn later() {}
}
";
        let once = add_regression_tags(rust, &[5], "0.5").unwrap();
        assert!(once.contains(
            "    // acceptance: 0.5\n    // regression: 0.5\n    #[test]\n    fn appends"
        ));
        assert!(!once.contains("regression: 0.6"));
        // The index skips tests that already carry `regression:`, so a second
        // run passes no lines and changes nothing.
        assert_eq!(add_regression_tags(&once, &[], "0.5").unwrap(), once);

        let shell = "#!/usr/bin/env bash\n# covers: QA-03\n# suite: integration\n# acceptance: 0.3\n#\nset -u\n";
        let promoted = add_regression_tags(shell, &[1], "0.3").unwrap();
        assert!(promoted.contains("# acceptance: 0.3\n# regression: 0.3\n#\nset -u\n"));

        assert!(add_regression_tags(rust, &[5], "0.9").is_err());

        // A shell test's own header wins over tag-like lines in its body.
        let shell_body = "#!/bin/sh\n# covers: QA-03\n# suite: unit\n# acceptance: 0.3\n\ncat <<'X'\n# acceptance: 0.3\nX\n";
        let promoted = add_regression_tags(shell_body, &[1], "0.3").unwrap();
        assert!(promoted.starts_with(
            "#!/bin/sh\n# covers: QA-03\n# suite: unit\n# acceptance: 0.3\n# regression: 0.3\n\n"
        ));
        assert_eq!(promoted.matches("regression").count(), 1);

        // CRLF files stay CRLF.
        let crlf =
            "// covers: infra\r\n// suite: unit\r\n// acceptance: 0.5\r\n#[test]\r\nfn t() {}\r\n";
        let promoted = add_regression_tags(crlf, &[4], "0.5").unwrap();
        assert!(promoted.contains("// acceptance: 0.5\r\n// regression: 0.5\r\n#[test]"));
        assert!(!promoted.replace("\r\n", "").contains('\n'));
    }
}
