//! Tests that keep the GitHub workflows honest: `ci-pass` waits for every
//! required job, every third-party action is pinned to a commit, and a
//! workflow calling another grants what the called jobs ask for.
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
    // Gates `ui` instead: if it fails, `ui` is skipped and ci-pass fails.
    "approval-reset",
];

#[derive(Debug, Deserialize)]
struct Workflow {
    permissions: Option<Permissions>,
    jobs: BTreeMap<String, Job>,
}

/// `permissions: read-all` or `permissions: { contents: read, … }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Permissions {
    All(String),
    Scopes(BTreeMap<String, String>),
}

impl Permissions {
    /// 0 none, 1 read, 2 write, for one scope.
    fn level(&self, scope: &str) -> u8 {
        let named = |level: &str| match level {
            "write" | "write-all" => 2,
            "read" | "read-all" => 1,
            _ => 0,
        };
        match self {
            Self::All(level) => named(level),
            Self::Scopes(scopes) => scopes.get(scope).map_or(0, |l| named(l)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Job {
    needs: Option<OneOrMany>,
    permissions: Option<Permissions>,
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

/// Jobs that other jobs need but that skip themselves depending on the
/// event (a job-level `if:` reading `github.`). GitHub skips every job
/// below a skipped one that has no `if:` of its own, even through a job in
/// between that did run, so ci-pass fails on those events. Such a job puts
/// its condition on its steps instead.
fn skipped_dependency_problems(workflow: &Workflow) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, job) in &workflow.jobs {
        let needs = job
            .needs
            .as_ref()
            .map(OneOrMany::to_set)
            .unwrap_or_default();
        for dep in needs {
            let skips = workflow
                .jobs
                .get(dep)
                .and_then(|d| d.condition.as_deref())
                .is_some_and(|c| c.contains("github."));
            if skips {
                problems.push(format!(
                    "`{name}` needs `{dep}`, which skips itself by event; \
                     move its `if:` to its steps"
                ));
            }
        }
    }
    problems
}

/// A job calling a reusable workflow must grant at least what each called
/// job asks for, or GitHub refuses to start the run (`startup_failure`).
/// `called` reads a local workflow by its `uses:` path.
fn caller_permission_problems(
    name: &str,
    workflow: &Workflow,
    called: impl Fn(&str) -> Option<Workflow>,
) -> Vec<String> {
    let mut problems = Vec::new();
    for (job_name, job) in &workflow.jobs {
        let Some(uses) = job.uses.as_deref().filter(|u| u.starts_with("./")) else {
            continue;
        };
        // Without any `permissions`, the repository default applies: unknown.
        let Some(granted) = job.permissions.as_ref().or(workflow.permissions.as_ref()) else {
            continue;
        };
        let Some(target) = called(uses) else {
            problems.push(format!(
                "{name} `{job_name}` calls {uses}, which can't be read"
            ));
            continue;
        };
        for (called_name, called_job) in &target.jobs {
            let wanted = called_job
                .permissions
                .as_ref()
                .or(target.permissions.as_ref());
            let Some(Permissions::Scopes(scopes)) = wanted else {
                continue;
            };
            for (scope, level) in scopes {
                let need = Permissions::All(level.clone()).level(scope);
                if granted.level(scope) < need {
                    problems.push(format!(
                        "{name} `{job_name}` grants less than `{scope}: {level}`, which {uses} `{called_name}` asks for"
                    ));
                }
            }
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
fn callers_grant_what_called_workflows_ask_for() {
    // main.yml called ci.yml with `contents: read` after ci.yml's pr-report
    // asked for `contents: write`: every main run failed to start.
    let root = root();
    let mut problems = Vec::new();
    for path in yaml_files(&root.join(".github/workflows")) {
        let workflow: Workflow = parse(&path);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        problems.extend(caller_permission_problems(&name, &workflow, |uses| {
            let file = root.join(uses.trim_start_matches("./"));
            file.is_file().then(|| parse(&file))
        }));
    }
    assert_eq!(problems, Vec::<String>::new());

    let called = "jobs:\n  report:\n    permissions:\n      contents: write\n      pull-requests: read\n  build: {}\n";
    let caller = |yaml: &str| -> Workflow { serde_saphyr::from_str(yaml).unwrap() };
    let check = |workflow: &Workflow| {
        caller_permission_problems("main.yml", workflow, |_| {
            Some(serde_saphyr::from_str(called).unwrap())
        })
    };
    let short = caller("permissions:\n  contents: read\njobs:\n  ci:\n    uses: ./ci.yml\n");
    assert_eq!(
        check(&short),
        [
            "main.yml `ci` grants less than `contents: write`, which ./ci.yml `report` asks for",
            "main.yml `ci` grants less than `pull-requests: read`, which ./ci.yml `report` asks for",
        ]
    );
    let enough = caller(
        "permissions:\n  contents: read\njobs:\n  ci:\n    uses: ./ci.yml\n    permissions: write-all\n",
    );
    assert_eq!(check(&enough), Vec::<String>::new());
    let unknown = caller("jobs:\n  ci:\n    uses: ./ci.yml\n");
    assert_eq!(check(&unknown), Vec::<String>::new());
    let missing = caller_permission_problems("main.yml", &short, |_| None);
    assert_eq!(
        missing,
        ["main.yml `ci` calls ./ci.yml, which can't be read"]
    );
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
#[test]
fn no_needed_job_skips_itself_by_event() {
    let workflow: Workflow = parse(&root().join(CI));
    assert_eq!(skipped_dependency_problems(&workflow), Vec::<String>::new());
}

// covers: QA-01
// suite: unit
#[test]
fn skipped_dependency_check_flags_event_conditions_only() {
    let workflow: Workflow = serde_saphyr::from_str(
        "jobs:\n  reset:\n    if: github.event_name == 'pull_request'\n  \
         ui:\n    needs: reset\n    if: ${{ !cancelled() }}\n  \
         trace:\n    needs: [ui]\n",
    )
    .unwrap();
    assert_eq!(
        skipped_dependency_problems(&workflow),
        ["`ui` needs `reset`, which skips itself by event; move its `if:` to its steps"]
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
