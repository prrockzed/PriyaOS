//! `cargo xtask coverage` (sub-phase 0.4, QA-06): changed code must be
//! tested. Lines added or changed against the base branch are matched with
//! the line coverage of a test run, per group from `xtask/coverage.toml`
//! (85 % by default, 90 % for the core crates).
//!
//! Only Rust is gated in Phase 0. Test code (`#[cfg(test)]` items, `#[test]`
//! functions, `tests/` folders) and generated files don't count. A PR
//! labelled `coverage-exception` whose description explains why under
//! "## Coverage justification" passes with a warning.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use syn::visit::Visit;

use crate::cmd::{cargo, read, relative, run};
use crate::scope;
use crate::scope::Workspace;
use crate::tags::rust::{marks_test_code, module_tree};

const CONFIG: &str = "xtask/coverage.toml";
const LCOV: &str = "target/coverage/lcov.info";
const REPORT: &str = "target/coverage/report.md";
pub const EXCEPTION_LABEL: &str = "coverage-exception";
/// The report's list of changed files with no coverage data.
const NOT_MEASURED: &str = "not measured";

pub struct Options {
    /// The branch the change is compared with, e.g. `origin/main`.
    pub base: String,
    /// An existing lcov report; without one, the tests run under llvm-cov.
    pub lcov: Option<PathBuf>,
    /// The pull request's labels and description, for the exception.
    pub labels: Vec<String>,
    pub pr_body: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Group {
    name: String,
    patterns: Vec<String>,
    threshold: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct Config {
    groups: Vec<Group>,
    default_threshold: f64,
    exclude: Vec<String>,
}

/// Changed, instrumented lines of one group.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Tally {
    covered: usize,
    total: usize,
    /// `file:line` of each uncovered line.
    missed: Vec<String>,
}

pub fn run_coverage(root: &Path, options: &Options) -> Result<()> {
    let config = config(&read(&root.join(CONFIG))?)?;
    let lcov_path = match &options.lcov {
        Some(path) => path.clone(),
        None => {
            measure(root)?;
            root.join(LCOV)
        }
    };
    let coverage = lcov(&read(&lcov_path)?, root);
    let changed = changed_lines(root, &options.base)?;
    let changed = without_test_code(root, changed)?;
    let tallies = tally(&config, &changed, &coverage);

    let failing = failing_groups(&config, &tallies);
    let exception = exception_applies(&options.labels, &options.pr_body);
    let report = report(&config, &tallies, &failing, exception);
    let path = root.join(REPORT);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).context("creating target/coverage")?;
    }
    std::fs::write(&path, &report).context("writing the coverage report")?;
    println!("{report}");

    if failing.is_empty() {
        return Ok(());
    }
    if exception {
        eprintln!(
            "warning: diff coverage is below the bar; passing because of `{EXCEPTION_LABEL}`"
        );
        return Ok(());
    }
    bail!(
        "diff coverage is below the bar in: {}. Add tests, or label the PR `{EXCEPTION_LABEL}` and explain why under \"## Coverage justification\"",
        failing.join(", ")
    );
}

/// Runs the tests under llvm-cov, writing `target/coverage/lcov.info`.
fn measure(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root.join("target/coverage")).context("creating target/coverage")?;
    run(
        "tests with coverage",
        cargo(root)
            .args([
                "llvm-cov",
                "nextest",
                "--locked",
                "--lcov",
                "--output-path",
                LCOV,
            ])
            .args(scope::for_this_os(root)?),
    )
}

fn config(toml: &str) -> Result<Config> {
    let doc: toml_edit::DocumentMut = toml.parse().context("reading xtask/coverage.toml")?;
    let number = |v: &toml_edit::Item| v.as_float().or_else(|| v.as_integer().map(|i| i as f64));
    let strings = |v: Option<&toml_edit::Item>| -> Vec<String> {
        v.and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let default_threshold = doc
        .get("default_threshold")
        .and_then(number)
        .context("coverage.toml: `default_threshold` is missing")?;
    let mut groups = Vec::new();
    if let Some(tables) = doc.get("group").and_then(|g| g.as_array_of_tables()) {
        for table in tables {
            let name = table
                .get("name")
                .and_then(|v| v.as_str())
                .context("coverage.toml: a group needs a `name`")?;
            if name == "default" {
                bail!("coverage.toml: `default` is the fallback group's name; pick another");
            }
            groups.push(Group {
                name: name.to_owned(),
                patterns: strings(table.get("paths")),
                threshold: table
                    .get("threshold")
                    .and_then(number)
                    .context("coverage.toml: a group needs a `threshold`")?,
            });
        }
    }
    Ok(Config {
        groups,
        default_threshold,
        exclude: strings(doc.get("exclude")),
    })
}

/// Path (repository-relative) → line → hit count, from an lcov report.
fn lcov(text: &str, root: &Path) -> BTreeMap<String, BTreeMap<usize, u64>> {
    let mut files: BTreeMap<String, BTreeMap<usize, u64>> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            current = Some(relative(root, Path::new(path)));
        } else if let (Some(file), Some(data)) = (&current, line.strip_prefix("DA:")) {
            let mut parts = data.split(',');
            let line_no = parts.next().and_then(|n| n.parse().ok());
            let hits: Option<u64> = parts.next().and_then(|n| n.parse().ok());
            if let (Some(line_no), Some(hits)) = (line_no, hits) {
                *files
                    .entry(file.clone())
                    .or_default()
                    .entry(line_no)
                    .or_default() += hits;
            }
        } else if line == "end_of_record" {
            current = None;
        }
    }
    files
}

/// Lines added or changed in Rust files since the merge base with `base`,
/// including uncommitted work and new files git doesn't track yet.
fn changed_lines(root: &Path, base: &str) -> Result<BTreeMap<String, BTreeSet<usize>>> {
    let merge_base = git(root, &["merge-base", base, "HEAD"])?;
    // Fixed prefixes and unquoted paths, whatever the user's git config says
    // (diff.noprefix, diff.mnemonicPrefix, core.quotePath).
    let diff = git(
        root,
        &[
            "-c",
            "core.quotePath=false",
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            merge_base.trim(),
            "--",
            "*.rs",
        ],
    )?;
    let mut changed = parse_diff(&diff);
    let untracked = git(
        root,
        &["ls-files", "--others", "--exclude-standard", "--", "*.rs"],
    )?;
    for file in untracked.lines() {
        let bytes = std::fs::read(root.join(file)).with_context(|| format!("reading {file}"))?;
        let count = String::from_utf8_lossy(&bytes).lines().count();
        changed.insert(file.to_owned(), (1..=count).collect());
    }
    Ok(changed)
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = crate::cmd::git(root)
        .args(args)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("git printed something that isn't UTF-8")
}

/// The new-side line numbers of each file in a `git diff --unified=0`.
fn parse_diff(diff: &str) -> BTreeMap<String, BTreeSet<usize>> {
    let mut changed: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut file: Option<String> = None;
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ ") {
            // `/dev/null` for deletions; git ends a name with spaces with a tab.
            file = path
                .strip_prefix("b/")
                .map(|p| p.trim_end_matches('\t').to_owned());
        } else if let (Some(file), Some(hunk)) = (&file, line.strip_prefix("@@ ")) {
            // `@@ -a,b +c,d @@`: d lines starting at c (d defaults to 1).
            let Some(new) = hunk.split_whitespace().find_map(|p| p.strip_prefix('+')) else {
                continue;
            };
            let mut parts = new.split(',');
            let start: usize = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            let count: usize = parts.next().and_then(|n| n.parse().ok()).unwrap_or(1);
            changed
                .entry(file.clone())
                .or_default()
                .extend(start..start + count);
        }
    }
    changed.retain(|_, lines| !lines.is_empty());
    changed
}

/// Drops test code: whole files that only tests compile (`#[cfg(test)]
/// mod x;` and everything below it), and in other files the lines of
/// test-only items (`#[cfg(test)]`, `#[cfg(all(test, …))]`, `#[test]`,
/// `#[tokio::test]`). Files that don't parse keep all their lines.
fn without_test_code(
    root: &Path,
    mut changed: BTreeMap<String, BTreeSet<usize>>,
) -> Result<BTreeMap<String, BTreeSet<usize>>> {
    let test_only = test_only_files(root)?;
    changed.retain(|file, _| !test_only.contains(file));
    for (file, lines) in &mut changed {
        let Ok(source) = std::fs::read_to_string(root.join(file)) else {
            continue;
        };
        let Ok(syntax) = syn::parse_file(&source) else {
            continue;
        };
        for range in TestCode::of(&syntax).ranges {
            lines.retain(|l| !range.contains(l));
        }
    }
    Ok(changed)
}

/// Repository-relative paths of Rust files only tests compile, from the
/// same module walk the test index uses (so `#[path]`, inline modules and
/// custom target roots are followed exactly).
fn test_only_files(root: &Path) -> Result<BTreeSet<String>> {
    let workspace = Workspace::load(root)?;
    let (files, _problems) = module_tree(&workspace);
    Ok(files
        .iter()
        .filter(|f| f.test_only)
        .map(|f| relative(root, &f.path))
        .collect())
}

/// The test code in one file: line ranges of test-only items.
#[derive(Default)]
struct TestCode {
    ranges: Vec<RangeInclusive<usize>>,
}

impl TestCode {
    fn of(file: &syn::File) -> Self {
        let mut code = Self::default();
        if file.attrs.iter().any(marks_test_code) {
            code.ranges.push(1..=usize::MAX); // `#![cfg(test)]`
        }
        code.visit_file(file);
        code
    }

    /// Records the item's lines when it is test code; true if it was.
    fn add(&mut self, attrs: &[syn::Attribute], end: proc_macro2::Span) -> bool {
        if !attrs.iter().any(marks_test_code) {
            return false;
        }
        let first = attrs
            .iter()
            .map(|a| a.pound_token.span.start().line)
            .min()
            .unwrap_or(end.start().line);
        self.ranges.push(first..=end.end().line);
        true
    }
}

impl<'ast> Visit<'ast> for TestCode {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        // `mod x;` files are handled by `test_only_files`.
        if let Some((brace, _)) = &item.content
            && !self.add(&item.attrs, brace.span.close())
        {
            syn::visit::visit_item_mod(self, item);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.add(&item.attrs, item.block.brace_token.span.close());
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if !self.add(&item.attrs, item.brace_token.span.close()) {
            syn::visit::visit_item_impl(self, item);
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.add(&item.attrs, item.block.brace_token.span.close());
    }
}

/// Counts changed, instrumented lines per group. A file goes to the first
/// group that matches it, else to "default"; excluded files are skipped.
fn tally(
    config: &Config,
    changed: &BTreeMap<String, BTreeSet<usize>>,
    coverage: &BTreeMap<String, BTreeMap<usize, u64>>,
) -> BTreeMap<String, Tally> {
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    for (file, lines) in changed {
        if config.exclude.iter().any(|p| glob_match(p, file)) {
            continue;
        }
        let group = config
            .groups
            .iter()
            .find(|g| g.patterns.iter().any(|p| glob_match(p, file)))
            .map_or("default", |g| g.name.as_str());
        let Some(hits) = coverage.get(file) else {
            // Not compiled in this run (another platform), or a path
            // mismatch: either way, show it rather than pass silently.
            tallies
                .entry(NOT_MEASURED.to_owned())
                .or_default()
                .missed
                .push(file.clone());
            continue;
        };
        let tally = tallies.entry(group.to_owned()).or_default();
        for line in lines {
            if let Some(count) = hits.get(line) {
                tally.total += 1;
                if *count > 0 {
                    tally.covered += 1;
                } else {
                    tally.missed.push(format!("{file}:{line}"));
                }
            }
        }
    }
    tallies.retain(|group, t| t.total > 0 || group == NOT_MEASURED);
    tallies
}

fn threshold(config: &Config, group: &str) -> f64 {
    config
        .groups
        .iter()
        .find(|g| g.name == group)
        .map_or(config.default_threshold, |g| g.threshold)
}

fn percent(t: &Tally) -> f64 {
    100.0 * t.covered as f64 / t.total as f64
}

fn failing_groups(config: &Config, tallies: &BTreeMap<String, Tally>) -> Vec<String> {
    tallies
        .iter()
        .filter(|(group, _)| *group != NOT_MEASURED)
        .filter(|(group, t)| percent(t) < threshold(config, group))
        .map(|(group, _)| group.clone())
        .collect()
}

/// The label is set and the PR description says why under "## Coverage
/// justification" (or "Coverage", any heading level)
/// (HTML comments, such as the template's hint, don't count).
fn exception_applies(labels: &[String], pr_body: &str) -> bool {
    labels.iter().any(|l| l == EXCEPTION_LABEL) && !coverage_section(pr_body).is_empty()
}

fn coverage_section(body: &str) -> String {
    let mut in_section = false;
    let mut text = String::new();
    for line in body.lines() {
        // A Markdown heading: one or more `#` and a space (not `#123`).
        let hashes = line.len() - line.trim_start_matches('#').len();
        if hashes > 0 && line[hashes..].starts_with(' ') {
            let title = line[hashes..].trim().to_ascii_lowercase();
            in_section = title == "coverage" || title == "coverage justification";
        } else if in_section {
            text.push_str(line);
            text.push('\n');
        }
    }
    // Drop HTML comments.
    while let Some(start) = text.find("<!--") {
        let end = text[start..]
            .find("-->")
            .map_or(text.len(), |e| start + e + 3);
        text.replace_range(start..end, "");
    }
    text.trim().to_owned()
}

fn report(
    config: &Config,
    tallies: &BTreeMap<String, Tally>,
    failing: &[String],
    exception: bool,
) -> String {
    let mut md = String::from("### Diff coverage\n\n");
    if tallies.is_empty() {
        md.push_str("No changed Rust lines to measure.\n");
        return md;
    }
    md.push_str("| Group | Covered | Lines | Coverage | Needed |\n|---|---|---|---|---|\n");
    for (group, t) in tallies.iter().filter(|(g, _)| *g != NOT_MEASURED) {
        let mark = if failing.contains(group) { " ❌" } else { "" };
        let _ = writeln!(
            md,
            "| {group} | {} | {} | {:.1} %{mark} | {} % |",
            t.covered,
            t.total,
            // Rounded down, so 89.96 % never shows as a failing "90.0 %".
            (percent(t) * 10.0).floor() / 10.0,
            threshold(config, group)
        );
    }
    if let Some(unmeasured) = tallies.get(NOT_MEASURED) {
        md.push_str("\nChanged files with no coverage data (not built in this run):\n\n");
        for file in &unmeasured.missed {
            let _ = writeln!(md, "- `{file}`");
        }
    }
    let missed: Vec<&String> = tallies
        .iter()
        .filter(|(g, _)| *g != NOT_MEASURED)
        .flat_map(|(_, t)| &t.missed)
        .collect();
    if !missed.is_empty() {
        md.push_str("\nUncovered changed lines:\n\n");
        for line in missed.iter().take(50) {
            let _ = writeln!(md, "- `{line}`");
        }
        if missed.len() > 50 {
            let _ = writeln!(md, "- …and {} more", missed.len() - 50);
        }
    }
    if !failing.is_empty() && exception {
        md.push_str("\n⚠️ Below the bar; allowed by the `coverage-exception` label and the PR's justification.\n");
    }
    md
}

/// Glob match for repository paths: `**` spans folders, `*` stays inside
/// one name.
fn glob_match(pattern: &str, path: &str) -> bool {
    fn matches(p: &[&str], s: &[&str]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(&"**"), _) => matches(&p[1..], s) || (!s.is_empty() && matches(p, &s[1..])),
            (Some(part), Some(name)) => segment_match(part, name) && matches(&p[1..], &s[1..]),
            _ => false,
        }
    }
    let p: Vec<&str> = pattern.split('/').collect();
    let s: Vec<&str> = path.split('/').collect();
    matches(&p, &s)
}

/// `*` matches any run of characters within one path segment.
fn segment_match(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((head, tail)) => {
            name.starts_with(head)
                && (0..=name.len() - head.len())
                    .filter(|&i| name.is_char_boundary(head.len() + i))
                    .any(|i| segment_match(tail, &name[head.len() + i..]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG_TOML: &str = r#"
default_threshold = 85
exclude = ["**/tests/**", "ui/PriyaUI/Theme.qml"]

[[group]]
name = "core"
paths = ["crates/priya-core/**", "crates/priya-ai/**"]
threshold = 90
"#;

    fn lines(spec: &[(&str, &[usize])]) -> BTreeMap<String, BTreeSet<usize>> {
        spec.iter()
            .map(|(f, l)| ((*f).to_owned(), l.iter().copied().collect()))
            .collect()
    }

    /// Coverage where `hit` lines ran and `miss` lines didn't.
    fn cov(spec: &[(&str, &[usize], &[usize])]) -> BTreeMap<String, BTreeMap<usize, u64>> {
        spec.iter()
            .map(|(f, hit, miss)| {
                let mut m: BTreeMap<usize, u64> = hit.iter().map(|l| (*l, 1)).collect();
                m.extend(miss.iter().map(|l| (*l, 0)));
                ((*f).to_owned(), m)
            })
            .collect()
    }

    // covers: QA-06
    // suite: unit
    // acceptance: 0.4
    #[test]
    fn coverage_groups_apply_stricter_threshold_to_core() {
        let config = config(CONFIG_TOML).unwrap();
        // 88 % in both groups: enough for "default" (85), not for "core" (90).
        let hit: Vec<usize> = (1..=22).collect();
        let miss: Vec<usize> = (23..=25).collect();
        let changed = lines(&[
            (
                "crates/priya-core/src/store.rs",
                &(1..=25).collect::<Vec<_>>(),
            ),
            ("crates/priyad/src/main.rs", &(1..=25).collect::<Vec<_>>()),
            ("crates/priya-core/tests/it.rs", &[1]),
        ]);
        let coverage = cov(&[
            ("crates/priya-core/src/store.rs", &hit, &miss),
            ("crates/priyad/src/main.rs", &hit, &miss),
            ("crates/priya-core/tests/it.rs", &[], &[1]),
        ]);
        let tallies = tally(&config, &changed, &coverage);
        assert_eq!(tallies["core"].total, 25);
        assert_eq!(tallies["default"].covered, 22);
        assert!(
            !tallies.contains_key("tests"),
            "tests/ folders are excluded"
        );
        assert_eq!(failing_groups(&config, &tallies), ["core"]);
    }

    // covers: QA-06
    // suite: unit
    // acceptance: 0.4
    #[test]
    fn coverage_exception_requires_label_and_justification() {
        let label = vec![EXCEPTION_LABEL.to_owned()];
        let template = "## Coverage\n\n<!-- Only if diff coverage is below the bar. -->\n\n## Design records\n";
        let justified =
            "## Coverage\n\nThe new branch only runs on Linux.\n\n## Design records\nnone\n";
        assert!(exception_applies(&label, justified));
        assert!(
            !exception_applies(&label, template),
            "the template hint isn't a reason"
        );
        assert!(!exception_applies(&[], justified), "the label is needed");
        assert!(!exception_applies(&label, "no section at all"));
        let other_heading = "### Coverage justification\r\nGenerated code.\r\n# Next\r\n";
        assert!(exception_applies(&label, other_heading));
    }

    // covers: QA-06
    // suite: unit
    #[test]
    fn diffs_and_lcov_are_read_per_line() {
        let diff = "diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n@@ -3,0 +4,2 @@\n+a\n+b\n@@ -10 +12 @@\n-c\n+d\n@@ -20,3 +21,0 @@\n--- a/gone.rs\n+++ /dev/null\n@@ -1,2 +0,0 @@\n";
        assert_eq!(parse_diff(diff), lines(&[("x.rs", &[4, 5, 12])]));
        let spaced = "+++ b/a b.rs\t\n@@ -0,0 +1 @@\n";
        assert_eq!(parse_diff(spaced), lines(&[("a b.rs", &[1])]));
        assert!(
            config("default_threshold = 85\n[[group]]\nname = \"default\"\nthreshold = 90\n")
                .is_err()
        );

        let root = Path::new("/repo");
        let report = "SF:/repo/crates/a/src/lib.rs\nDA:3,0\nDA:4,7\nend_of_record\n";
        let parsed = lcov(report, root);
        assert_eq!(parsed["crates/a/src/lib.rs"][&4], 7);
        assert_eq!(parsed["crates/a/src/lib.rs"][&3], 0);
    }

    // covers: QA-06
    // suite: unit
    #[test]
    fn test_code_does_not_count() {
        let source = "pub fn real() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n\n#[test]\nfn loose() {\n}\n\n#[cfg(all(test, unix))]\nimpl S {\n    fn h() {}\n}\n\nimpl T {\n    #[tokio::test]\n    async fn a() {}\n    fn b() {}\n}\n\n#[cfg(any(test, feature = \"x\"))]\nfn shared() {}\n";
        let ranges = TestCode::of(&syn::parse_file(source).unwrap()).ranges;
        let is_test = |l: usize| ranges.iter().any(|r| r.contains(&l));
        assert!(!is_test(1));
        assert!((3..=7).all(is_test), "{ranges:?}");
        assert!((9..=11).all(is_test));
        assert!((13..=16).all(is_test), "cfg(all(test, …)) on an impl");
        assert!((19..=20).all(is_test), "#[tokio::test] inside an impl");
        assert!(!is_test(21), "an ordinary method stays");
        assert!(
            !is_test(25),
            "cfg(any(test, …)) is also built without tests"
        );
    }

    // covers: QA-06
    // suite: unit
    #[test]
    fn files_only_tests_compile_are_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\"]\nresolver = \"3\"\n",
        );
        // A binary with a custom root path: its folder rules come from cargo
        // metadata, not from guessing.
        write(
            "crates/a/Cargo.toml",
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[[bin]]\nname = \"run\"\npath = \"tools/run.rs\"\n",
        );
        write(
            "crates/a/src/lib.rs",
            "mod real;\nmod inner_only;\n#[cfg(test)]\nmod helpers;\n#[cfg(all(test, unix))]\n#[path = \"odd/place.rs\"]\nmod placed;\n",
        );
        write("crates/a/src/real.rs", "pub fn r() {}\n");
        write("crates/a/src/helpers.rs", "mod deeper;\n");
        write("crates/a/src/helpers/deeper.rs", "pub fn d() {}\n");
        write("crates/a/src/odd/place.rs", "");
        write("crates/a/src/inner_only.rs", "#![cfg(test)]\nmod x;\n");
        write("crates/a/src/inner_only/x.rs", "");
        write(
            "crates/a/tools/run.rs",
            "#[cfg(test)]\nmod t;\nfn main() {}\n",
        );
        write("crates/a/tools/t.rs", "");
        let found = test_only_files(&root).unwrap();
        assert_eq!(
            found.into_iter().collect::<Vec<_>>(),
            [
                "crates/a/src/helpers.rs",
                "crates/a/src/helpers/deeper.rs",
                "crates/a/src/inner_only.rs",
                "crates/a/src/inner_only/x.rs",
                "crates/a/src/odd/place.rs",
                "crates/a/tools/t.rs",
            ]
        );
    }

    // covers: QA-06
    // suite: unit
    #[test]
    fn globs_match_paths() {
        assert!(glob_match(
            "crates/priya-core/**",
            "crates/priya-core/src/a/b.rs"
        ));
        assert!(glob_match("**/tests/**", "crates/x/tests/it.rs"));
        assert!(glob_match("docs/claude/api/**", "docs/claude/api/v1.md"));
        assert!(glob_match("ui/*/Theme.qml", "ui/PriyaUI/Theme.qml"));
        assert!(!glob_match(
            "crates/priya-core/**",
            "crates/priya-corex/src/a.rs"
        ));
        assert!(!glob_match("ui/*/Theme.qml", "ui/a/b/Theme.qml"));
    }

    // covers: QA-06
    // suite: integration
    // acceptance: 0.4
    #[test]
    fn xtask_coverage_on_fixture_diff() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let git = |args: &[&str]| {
            let ok = crate::cmd::git(&root)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args(args)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        git(&["init", "-q"]);
        // Settings that change `git diff`'s output must not change the result.
        git(&["config", "diff.mnemonicPrefix", "true"]);
        git(&["config", "diff.noprefix", "true"]);
        git(&["config", "core.quotePath", "true"]);
        write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/priya-core\"]\nresolver = \"3\"\n",
        );
        write(
            "crates/priya-core/Cargo.toml",
            "[package]\nname = \"priya-core\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        write(CONFIG, CONFIG_TOML);
        write("crates/priya-core/src/lib.rs", "pub fn a() {}\n");
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "base"]);

        // The change adds lines 2-4; the lcov report says line 4 never ran.
        write(
            "crates/priya-core/src/lib.rs",
            "pub fn a() {}\npub fn b() {\n    x();\n}\n",
        );
        let lcov_for = |line4: u64| {
            let path = root.join("lcov.info");
            let file = root.join("crates/priya-core/src/lib.rs");
            std::fs::write(
                &path,
                format!(
                    "SF:{}\nDA:2,1\nDA:3,1\nDA:4,{line4}\nend_of_record\n",
                    file.display()
                ),
            )
            .unwrap();
            path
        };
        let options = |lcov: PathBuf, labels: Vec<String>, body: &str| Options {
            base: "HEAD".into(),
            lcov: Some(lcov),
            labels,
            pr_body: body.into(),
        };

        let err = run_coverage(&root, &options(lcov_for(0), Vec::new(), "")).unwrap_err();
        assert!(err.to_string().contains("below the bar in: core"), "{err}");
        let report = std::fs::read_to_string(root.join(REPORT)).unwrap();
        assert!(
            report.contains("| core | 2 | 3 | 66.6 % ❌ | 90 % |"),
            "{report}"
        );
        assert!(report.contains("`crates/priya-core/src/lib.rs:4`"));

        let excused = options(
            lcov_for(0),
            vec![EXCEPTION_LABEL.into()],
            "## Coverage\nLinux only.\n",
        );
        run_coverage(&root, &excused).unwrap();
        run_coverage(&root, &options(lcov_for(1), Vec::new(), "")).unwrap();

        // A new file git doesn't track yet counts in full.
        write("crates/priya-core/src/new.rs", "pub fn n() {}\n");
        let new_file = root.join("crates/priya-core/src/new.rs");
        let lcov = lcov_for(1);
        let mut report = std::fs::read_to_string(&lcov).unwrap();
        report.push_str(&format!(
            "SF:{}\nDA:1,0\nend_of_record\n",
            new_file.display()
        ));
        std::fs::write(&lcov, report).unwrap();
        let err = run_coverage(&root, &options(lcov, Vec::new(), "")).unwrap_err();
        assert!(err.to_string().contains("core"), "{err}");
        let report = std::fs::read_to_string(root.join(REPORT)).unwrap();
        assert!(
            report.contains("`crates/priya-core/src/new.rs:1`"),
            "{report}"
        );

        // Changed files the coverage run didn't build are listed, not hidden
        // (and don't fail the run: they may be built only on another OS).
        write("crates/other/src/lib.rs", "pub fn o() {}\n");
        run_coverage(&root, &options(lcov_for(1), Vec::new(), "")).unwrap();
        let report = std::fs::read_to_string(root.join(REPORT)).unwrap();
        assert!(
            report.contains("Changed files with no coverage data"),
            "{report}"
        );
        assert!(report.contains("- `crates/other/src/lib.rs`"), "{report}");
    }
}
