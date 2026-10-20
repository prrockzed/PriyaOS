//! Tests that keep the GitHub workflows honest: `ci-pass` waits for every
//! required job, and every third-party action is pinned to a commit.
//!
//! Branch protection requires only `ci-pass`, so a job missing from its
//! `needs` could fail without blocking a merge.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const CI: &str = ".github/workflows/ci.yml";
const AGGREGATOR: &str = "ci-pass";
/// Jobs in ci.yml that may fail without blocking a merge. Each needs a reason.
const NOT_REQUIRED: &[&str] = &[
    // A comment for people; its inputs are gated by their own jobs.
    "pr-report",
];

#[derive(Debug, Deserialize)]
struct Workflow {
    jobs: BTreeMap<String, Job>,
}

#[derive(Debug, Deserialize)]
struct Job {
    needs: Option<OneOrMany>,
    #[serde(rename = "if")]
    condition: Option<String>,
    /// A job that calls a reusable workflow.
    uses: Option<String>,
    container: Option<Container>,
    #[serde(default)]
    steps: Vec<Step>,
}

/// `container: image` or `container: { image: … }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Container {
    Image(String),
    Full { image: String },
}

impl Container {
    fn image(&self) -> &str {
        match self {
            Self::Image(image) | Self::Full { image } => image,
        }
    }
}

/// A composite action (`.github/actions/*/action.yml`).
#[derive(Debug, Deserialize)]
struct Action {
    runs: Runs,
}

#[derive(Debug, Deserialize)]
struct Runs {
    #[serde(default)]
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
struct Step {
    uses: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn to_set(&self) -> BTreeSet<&str> {
        match self {
            Self::One(job) => BTreeSet::from([job.as_str()]),
            Self::Many(jobs) => jobs.iter().map(String::as_str).collect(),
        }
    }
}

/// Why the aggregator doesn't guard the workflow, if it doesn't.
fn aggregator_problems(workflow: &Workflow) -> Vec<String> {
    let Some(aggregator) = workflow.jobs.get(AGGREGATOR) else {
        return vec![format!("there is no `{AGGREGATOR}` job")];
    };
    let mut problems = Vec::new();
    // Without always(), a failed dependency skips ci-pass, and GitHub treats
    // a skipped required check as passing.
    if aggregator.condition.as_deref().map(str::trim) != Some("always()") {
        problems.push(format!("`{AGGREGATOR}` must run with `if: always()`"));
    }
    let needs = aggregator
        .needs
        .as_ref()
        .map(OneOrMany::to_set)
        .unwrap_or_default();
    for job in workflow.jobs.keys() {
        let job = job.as_str();
        if job != AGGREGATOR && !NOT_REQUIRED.contains(&job) && !needs.contains(job) {
            problems.push(format!("`{AGGREGATOR}` does not need `{job}`"));
        }
    }
    for job in needs {
        if !workflow.jobs.contains_key(job) {
            problems.push(format!("`{AGGREGATOR}` needs unknown job `{job}`"));
        }
    }
    problems
}

/// Local actions and workflows are fine; anything else must name a full
/// commit SHA, because tags can be moved to point at other code.
fn is_pinned(uses: &str) -> bool {
    if uses.starts_with("./") {
        return true;
    }
    let Some((action, reference)) = uses.rsplit_once('@') else {
        return false;
    };
    action.contains('/')
        && reference.len() == 40
        && reference
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Every `uses:` in the repository's workflows and composite actions, with
/// the file it came from.
fn all_uses(root: &Path) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    for path in yaml_files(&root.join(".github/workflows")) {
        let workflow: Workflow = parse(&path);
        for job in workflow.jobs.values() {
            let step_uses = job.steps.iter().filter_map(|s| s.uses.as_ref());
            for uses in job.uses.iter().chain(step_uses) {
                found.push((path.clone(), uses.clone()));
            }
        }
    }
    for dir in subdirs(&root.join(".github/actions")) {
        // GitHub accepts either name for a composite action.
        let Some(path) = ["action.yml", "action.yaml"]
            .iter()
            .map(|name| dir.join(name))
            .find(|p| p.exists())
        else {
            panic!("{} has no action.yml", dir.display());
        };
        let action: Action = parse(&path);
        for uses in action.runs.steps.iter().filter_map(|s| s.uses.as_ref()) {
            found.push((path.clone(), uses.clone()));
        }
    }
    found
}

fn parse<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_saphyr::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = entries(dir)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "yml" || e == "yaml"))
        .collect();
    files.sort();
    files
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = entries(dir).into_iter().filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|it| it.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default()
}

fn root() -> PathBuf {
    crate::cmd::workspace_root().unwrap()
}

// covers: QA-01
// suite: unit
// acceptance: 0.3
// regression: 0.3
#[test]
fn ci_pass_needs_every_required_job() {
    let workflow: Workflow = parse(&root().join(CI));
    assert_eq!(aggregator_problems(&workflow), Vec::<String>::new());
}

// covers: QA-01
// suite: unit
#[test]
fn aggregator_check_catches_a_missing_job_and_a_missing_always() {
    let workflow: Workflow = serde_saphyr::from_str(
        "jobs:\n  fmt: {}\n  test: {}\n  ci-pass:\n    needs: [fmt, gone]\n",
    )
    .unwrap();
    assert_eq!(
        aggregator_problems(&workflow),
        [
            "`ci-pass` must run with `if: always()`",
            "`ci-pass` does not need `test`",
            "`ci-pass` needs unknown job `gone`",
        ]
    );
}

// covers: QA-01
// suite: unit
// acceptance: 0.3
// regression: 0.3
#[test]
fn workflow_actions_pinned_by_sha() {
    let uses = all_uses(&root());
    assert!(!uses.is_empty(), "found no actions; is the path right?");
    let unpinned: Vec<String> = uses
        .iter()
        .filter(|(_, u)| !is_pinned(u))
        .map(|(path, u)| format!("{}: {u}", path.display()))
        .collect();
    assert_eq!(unpinned, Vec::<String>::new());
}

// covers: QA-01
// suite: unit
#[test]
fn pin_check_accepts_only_full_shas_and_local_paths() {
    let sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
    assert!(is_pinned(&format!("actions/checkout@{sha}")));
    assert!(is_pinned("./.github/actions/setup-rust"));
    assert!(!is_pinned("actions/checkout@v7"));
    assert!(!is_pinned("actions/checkout@main"));
    assert!(!is_pinned(&format!(
        "actions/checkout@{}",
        sha.to_uppercase()
    )));
    assert!(!is_pinned(&format!("checkout@{sha}")));
    assert!(!is_pinned("docker://alpine:3"));
}

// covers: QA-01
// suite: unit
#[test]
fn job_containers_are_pinned_by_digest() {
    let mut images = Vec::new();
    for path in yaml_files(&root().join(".github/workflows")) {
        let workflow: Workflow = parse(&path);
        for (name, job) in &workflow.jobs {
            if let Some(container) = &job.container {
                images.push((
                    format!("{}: {name}", path.display()),
                    container.image().to_owned(),
                ));
            }
        }
    }
    assert!(
        !images.is_empty(),
        "the Linux jobs should run in the CI image"
    );
    let floating: Vec<String> = images
        .iter()
        .filter(|(_, image)| !image.contains("@sha256:"))
        .map(|(job, image)| format!("{job}: {image}"))
        .collect();
    assert_eq!(floating, Vec::<String>::new());
}
