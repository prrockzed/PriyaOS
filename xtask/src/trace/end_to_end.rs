//! `cargo xtask trace` on a small, complete repository: a feature catalog,
//! a phase file, and tests in Rust, QML and shell, with results from a JUnit
//! report and the script results file. The matrix it writes is compared with
//! a golden copy.

use std::fs;
use std::path::Path;

use super::{Options, run_trace};

/// The repository, written into `root`.
const FILES: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/demo\"]\nresolver = \"3\"\n",
    ),
    (
        "crates/demo/Cargo.toml",
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    ),
    (
        "crates/demo/src/lib.rs",
        "#[cfg(test)]
mod tests {
    // covers: DEMO-01
    // suite: unit
    // acceptance: 9.1
    // regression: 9.1
    #[test]
    fn accepted() {}

    // covers: DEMO-02
    // suite: integration
    #[test]
    fn in_progress() {}
}
",
    ),
    (
        "ui/tests/tst_demo.qml",
        "TestCase {\n    // covers: DEMO-02\n    // suite: ui\n    function test_shows_it() {}\n}\n",
    ),
    (
        "scripts/tests/test_demo.sh",
        "#!/usr/bin/env bash\n# covers: DEMO-01\n# suite: integration\n\ntrue\n",
    ),
    (
        "docs/claude/features.md",
        "# Features

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| DEMO-01 | Done and tested | C | 9 | ✅ |
| DEMO-02 | Being built | C | 9 | 🟨 |
| DEMO-03 | Not started | C | 9 | ⬜ |
",
    ),
    (
        "docs/claude/phases/phase-09-demo.md",
        "# Phase 9

### 9.1 — The finished part
**Done when**
- [x] It works.

### 9.2 — The next part
**Done when**
- [ ] It works.
",
    ),
    ("docs/claude/architecture/decisions.md", "# Decisions\n"),
    (
        "run/junit.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run">
  <testsuite name="demo">
    <testcase name="tests::accepted" classname="demo"/>
    <testcase name="tests::in_progress" classname="demo"><failure>boom</failure></testcase>
  </testsuite>
</testsuites>
"#,
    ),
    (
        "run/script-results.json",
        "{\"scripts/tests/test_demo.sh\": \"passed\"}\n",
    ),
];

fn write_repo(root: &Path) {
    for (path, text) in FILES {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

// covers: QA-05
// suite: integration
// acceptance: 0.4
// regression: 0.4
#[test]
fn xtask_trace_end_to_end_on_fixture_repo() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write_repo(&root);

    let options = Options {
        junit: vec![root.join("run/junit.xml")],
        base: None,
        pr_body: String::new(),
        results: vec![root.join("run/script-results.json")],
    };
    run_trace(&root, &options).unwrap();
    let matrix = fs::read_to_string(root.join("target/trace/matrix.md")).unwrap();
    priya_testkit::assert_golden_text!("trace_matrix_of_fixture_repo", matrix);

    // The same repository with DEMO-01's only Rust test failing: its shell
    // test still passed, so rule 3 holds; failing that too breaks it.
    fs::write(
        root.join("run/script-results.json"),
        "{\"scripts/tests/test_demo.sh\": \"failed\"}\n",
    )
    .unwrap();
    let junit = fs::read_to_string(root.join("run/junit.xml"))
        .unwrap()
        .replace(
            r#"<testcase name="tests::accepted" classname="demo"/>"#,
            r#"<testcase name="tests::accepted" classname="demo"><failure/></testcase>"#,
        );
    fs::write(root.join("run/junit.xml"), junit).unwrap();
    let err = run_trace(&root, &options).unwrap_err();
    assert!(err.to_string().contains("1 traceability problem"), "{err}");
}
