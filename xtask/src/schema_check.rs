//! `cargo xtask schemas --check` (sub-phase 0.9): the committed schemas
//! match the code, and no change breaks what clients, old events, settings
//! files or modules rely on unless it comes with its version bump.
//!
//! Three trees are compared: what the code generates now, what is
//! committed, and what the base branch had. Generated and committed must be
//! identical. Each difference from the base is classified by
//! [`schema_diff`](crate::schema_diff); a breaking one fails unless:
//! - (a) public API: never in place; a new `v2.` method is added instead
//!   and the `v1.` one stays as it was;
//! - (b) events: never in place; a new version file comes next to the old
//!   one, with an upcaster from the old version and a fixture of it;
//! - (c) settings: `settings_version` is bumped, migrations lead from the
//!   old version, and old-config fixtures of it migrate to valid settings;
//! - (d) manifest and module protocol: the manifest `api` version is
//!   bumped.
//!
//! What the code registers (upcasters, migrations, fixtures) comes in as
//! [`Facts`], so the rules can be tested on synthetic trees.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::schema_diff::{Change, Class, Position, diff, diff_part};
use crate::schemas::{API_DIR, API_JS, Files, event_file, parse_event_file};

/// Where the fixtures of old event versions live: `<type>/v<N>.json`.
pub const EVENT_FIXTURES: &str = "crates/priya-api/tests/fixtures/events";
/// Where old settings files live: `v<N>/*.toml`.
pub const SETTINGS_FIXTURES: &str = "crates/priya-core/tests/fixtures/settings";

type Upcast = dyn Fn(&str, u16, Value) -> Result<(u16, Value), String>;
type ValidateEvent = dyn Fn(&str, u16, &Value) -> Result<(), String>;
type CheckSettings = dyn Fn(&str) -> Result<(), String>;

/// What the code registers, beyond the schemas.
pub struct Facts {
    /// `(type, from)` of every event upcaster.
    pub upcasters: BTreeSet<(String, u16)>,
    /// `from` of every settings migration.
    pub migrations: BTreeSet<u32>,
    /// Old event payloads, by `(type, version)`.
    pub event_fixtures: BTreeMap<(String, u16), Value>,
    /// Old settings files, by format version: `(name, text)`.
    pub settings_fixtures: BTreeMap<u32, Vec<(String, String)>>,
    /// Lifts a payload to its type's current version.
    pub upcast: Box<Upcast>,
    /// Checks a payload against its type and version.
    pub validate_event: Box<ValidateEvent>,
    /// Migrates an old settings file and checks the result.
    pub check_settings: Box<CheckSettings>,
}

impl Facts {
    /// What this build registers, and the fixtures in `root`.
    pub fn real(root: &Path) -> Result<Self> {
        use priya_core::settings::{Layer, Migrated, Source, load, migrate};
        Ok(Self {
            upcasters: priya_api::events::UPCASTERS
                .iter()
                .map(|u| (u.event_type.to_owned(), u.from))
                .collect(),
            migrations: priya_core::settings::MIGRATIONS
                .iter()
                .map(|m| m.from)
                .collect(),
            event_fixtures: event_fixtures(&root.join(EVENT_FIXTURES))?,
            settings_fixtures: settings_fixtures(&root.join(SETTINGS_FIXTURES))?,
            upcast: Box::new(|event_type, version, payload| {
                priya_api::events::upcast(event_type, version, payload).map_err(|e| e.to_string())
            }),
            validate_event: Box::new(|event_type, version, payload| {
                priya_api::events::kind(event_type, version)
                    .ok_or_else(|| format!("{event_type} v{version} isn't registered"))?
                    .validate(payload)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }),
            check_settings: Box::new(|text| {
                let text = match migrate(text).map_err(|e| e.to_string())? {
                    Migrated::Current => text.to_owned(),
                    Migrated::Upgraded { text, .. } => text,
                };
                load([Source::toml(Layer::User, "fixture.toml", text)], None)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }),
        })
    }
}

fn event_fixtures(dir: &Path) -> Result<BTreeMap<(String, u16), Value>> {
    let mut found = BTreeMap::new();
    for type_dir in sorted_entries(dir)?.into_iter().filter(|p| p.is_dir()) {
        let Some(event_type) = type_dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        for file in sorted_entries(&type_dir)? {
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let Some(version) = name
                .strip_prefix('v')
                .and_then(|n| n.strip_suffix(".json"))
                .and_then(|n| n.parse().ok())
            else {
                continue;
            };
            let text = fs::read_to_string(&file)?;
            let payload = serde_json::from_str(&text)
                .with_context(|| format!("{} isn't JSON", file.display()))?;
            found.insert((event_type.to_owned(), version), payload);
        }
    }
    Ok(found)
}

fn settings_fixtures(dir: &Path) -> Result<BTreeMap<u32, Vec<(String, String)>>> {
    let mut found = BTreeMap::new();
    for version_dir in sorted_entries(dir)?.into_iter().filter(|p| p.is_dir()) {
        let name = version_dir.file_name().and_then(|n| n.to_str());
        let Some(version) = name
            .and_then(|n| n.strip_prefix('v'))
            .and_then(|n| n.parse().ok())
        else {
            continue;
        };
        let mut files = Vec::new();
        for file in sorted_entries(&version_dir)? {
            if file.extension().is_some_and(|e| e == "toml") {
                let name = file.file_name().map(|n| n.to_string_lossy().into_owned());
                files.push((name.unwrap_or_default(), fs::read_to_string(&file)?));
            }
        }
        found.insert(version, files);
    }
    Ok(found)
}

fn sorted_entries(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut entries = match fs::read_dir(dir) {
        Ok(entries) => entries
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?
            .into_iter()
            // Hidden files, such as Finder's, are nobody's business.
            .filter(|p| {
                !p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
            })
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
    };
    entries.sort();
    Ok(entries)
}

/// One classified difference from the base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    /// Relative to [`API_DIR`].
    pub file: String,
    pub change: Change,
}

/// What `--check` found.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Generated files that differ from the committed ones.
    pub stale: Vec<String>,
    /// Every difference from the base; empty without one.
    pub changes: Vec<Classified>,
    /// Why the check fails, one sentence each.
    pub problems: Vec<String>,
    /// Whether a base was compared.
    pub compared: bool,
}

impl Outcome {
    pub fn passed(&self) -> bool {
        self.stale.is_empty() && self.problems.is_empty()
    }

    /// The PR report section.
    pub fn report(&self) -> String {
        let mut out = String::from("### Schema changes\n\n");
        if !self.stale.is_empty() {
            out.push_str("The committed schemas don't match the code; run `cargo xtask schemas` and commit:\n\n");
            for line in &self.stale {
                let _ = writeln!(out, "- {line}");
            }
            out.push('\n');
        }
        if !self.compared {
            out.push_str("Not compared with a base branch.\n");
        } else if self.changes.is_empty() {
            out.push_str("No schema changes.\n");
        } else {
            out.push_str("| File | Where | Change | Class |\n|---|---|---|---|\n");
            for c in &self.changes {
                let class = match c.change.class {
                    Class::Additive => "additive",
                    Class::Breaking => "**breaking**",
                };
                let _ = writeln!(
                    out,
                    "| `{}` | `{}` | {} | {class} |",
                    c.file,
                    c.change.path,
                    c.change.what.replace('|', "\\|")
                );
            }
        }
        if !self.problems.is_empty() {
            out.push_str("\nProblems:\n\n");
            for problem in &self.problems {
                let _ = writeln!(out, "- {problem}");
            }
        }
        out
    }
}

/// Compares the trees and applies the rules. Paths are repository-relative.
pub fn check(generated: &Files, committed: &Files, base: Option<&Files>, facts: &Facts) -> Outcome {
    let mut outcome = Outcome {
        stale: stale(generated, committed),
        ..Outcome::default()
    };
    let head = schemas(generated);
    if let Some(base) = base {
        outcome.compared = true;
        let base = schemas(base);
        classify(&base, &head, facts, &mut outcome);
    }
    old_events(&head, facts, &mut outcome.problems);
    for (version, files) in &facts.settings_fixtures {
        for (name, text) in files {
            if let Err(e) = (facts.check_settings)(text) {
                outcome.problems.push(format!(
                    "The old settings file {SETTINGS_FIXTURES}/v{version}/{name} doesn't migrate to valid settings: {e}"
                ));
            }
        }
    }
    outcome
}

/// What differs between generated and committed files.
pub fn stale(generated: &Files, committed: &Files) -> Vec<String> {
    let mut stale = Vec::new();
    for (path, text) in generated {
        match committed.get(path) {
            Some(old) if old == text => {}
            Some(_) => stale.push(format!("{path} is out of date")),
            None => stale.push(format!("{path} is missing")),
        }
    }
    for path in committed.keys().filter(|p| !generated.contains_key(*p)) {
        stale.push(format!("{path} is no longer generated"));
    }
    stale
}

/// The JSON schema files of a tree, relative to [`API_DIR`], parsed; files
/// that aren't JSON count as empty schemas (the stale check reports them).
fn schemas(files: &Files) -> BTreeMap<String, Value> {
    let prefix = format!("{API_DIR}/");
    files
        .iter()
        .filter(|(path, _)| path.ends_with(".json") && path.as_str() != API_JS)
        .filter_map(|(path, text)| {
            let rel = path.strip_prefix(&prefix)?;
            Some((
                rel.to_owned(),
                serde_json::from_str(text).unwrap_or(Value::Null),
            ))
        })
        .collect()
}

/// What kind of contract a schema file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Contract {
    Public,
    Errors,
    Event,
    CoreSettings,
    ModuleSettings,
    /// The manifest or the module protocol.
    Module,
}

fn contract(file: &str) -> Option<Contract> {
    Some(match file {
        "public/v1/errors.json" => Contract::Errors,
        _ if file.starts_with("public/") => Contract::Public,
        _ if file.starts_with("events/") => Contract::Event,
        "settings/core.json" => Contract::CoreSettings,
        _ if file.starts_with("settings/modules/") => Contract::ModuleSettings,
        _ if file.starts_with("module/") => Contract::Module,
        _ => return None,
    })
}

/// The changes in one file, with each part read the way its data flows.
fn file_changes(file: &str, old: &Value, new: &Value) -> Vec<Change> {
    let Some(contract) = contract(file) else {
        return Vec::new();
    };
    match contract {
        Contract::Public | Contract::Module if new.get("x-kind").is_some() => {
            // Whoever receives the params must still accept them: they are
            // input when priyad receives them, output when priyad sends.
            let (params, result) = match new["x-direction"].as_str() {
                Some("host_to_module" | "daemon_to_client") => (Position::Output, Position::Input),
                _ => (Position::Input, Position::Output),
            };
            let mut changes = metadata_changes(old, new);
            for (part, position) in [("params", params), ("result", result)] {
                let pointer = format!("/$defs/{part}");
                if old.pointer(&pointer).is_none() && new.pointer(&pointer).is_none() {
                    continue;
                }
                changes.extend(
                    diff_part(old, new, &pointer, position)
                        .into_iter()
                        .map(|mut c| {
                            c.path = format!("{part}{}", c.path.trim_end_matches('/'));
                            c
                        }),
                );
            }
            changes
        }
        Contract::Errors => {
            let mut changes = diff(old, new, Position::Output);
            let codes = |doc: &Value| doc["x-codes"].as_object().cloned().unwrap_or_default();
            let (old_codes, new_codes) = (codes(old), codes(new));
            for (kind, code) in &old_codes {
                match new_codes.get(kind) {
                    Some(same) if same == code => {}
                    Some(other) => changes.push(change(
                        "x-codes",
                        Class::Breaking,
                        format!("the code of `{kind}` changed from {code} to {other}"),
                    )),
                    None => changes.push(change(
                        "x-codes",
                        Class::Breaking,
                        format!("kind `{kind}` was removed"),
                    )),
                }
            }
            for kind in new_codes.keys().filter(|k| !old_codes.contains_key(*k)) {
                changes.push(change(
                    "x-codes",
                    Class::Additive,
                    format!("kind `{kind}` was added"),
                ));
            }
            changes
        }
        Contract::Event => diff(old, new, Position::Both),
        Contract::CoreSettings | Contract::ModuleSettings | Contract::Module => {
            diff(old, new, Position::Input)
        }
        Contract::Public => diff(old, new, Position::Output),
    }
}

/// A method that was read-only may be called by modules, and clients may
/// rely on its notification.
fn metadata_changes(old: &Value, new: &Value) -> Vec<Change> {
    let mut changes = Vec::new();
    match (old["x-read-only"].as_bool(), new["x-read-only"].as_bool()) {
        (Some(true), Some(false)) => changes.push(change(
            "x-read-only",
            Class::Breaking,
            "it is no longer read-only",
        )),
        (Some(false), Some(true)) => {
            changes.push(change(
                "x-read-only",
                Class::Additive,
                "it is now read-only",
            ));
        }
        _ => {}
    }
    for key in ["x-notification", "x-direction"] {
        if old.get(key) != new.get(key) {
            let class = if old.get(key).is_none() {
                Class::Additive
            } else {
                Class::Breaking
            };
            changes.push(change(key, class, format!("`{key}` changed")));
        }
    }
    changes
}

fn change(path: &str, class: Class, what: impl Into<String>) -> Change {
    Change {
        path: path.to_owned(),
        class,
        what: what.into(),
    }
}

/// The manifest API version a tree declares (`module/manifest.v<N>.json`).
fn manifest_version(tree: &BTreeMap<String, Value>) -> Option<u32> {
    tree.keys()
        .filter_map(|f| {
            f.strip_prefix("module/manifest.v")?
                .strip_suffix(".json")?
                .parse()
                .ok()
        })
        .max()
}

/// The `v<N>` in `module/manifest.v<N>.json` or `module/host-v<N>/…`.
fn module_version(file: &str) -> Option<u32> {
    let rest = file
        .strip_prefix("module/manifest.v")
        .and_then(|r| r.strip_suffix(".json"))
        .or_else(|| file.strip_prefix("module/host-v")?.split('/').next())?;
    rest.parse().ok()
}

fn settings_version(tree: &BTreeMap<String, Value>) -> Option<u64> {
    tree.get("settings/core.json")?["x-settings-version"].as_u64()
}

fn classify(
    base: &BTreeMap<String, Value>,
    head: &BTreeMap<String, Value>,
    facts: &Facts,
    outcome: &mut Outcome,
) {
    let mut broken_settings = false;
    let manifest_bumped = match (manifest_version(base), manifest_version(head)) {
        (Some(old), Some(new)) => new > old,
        _ => false,
    };
    for (file, old) in base {
        let Some(contract) = contract(file) else {
            continue;
        };
        let Some(new) = head.get(file) else {
            let allowed = contract == Contract::Module
                && manifest_bumped
                && module_version(file).is_some_and(|v| Some(v) < manifest_version(head));
            outcome.changes.push(Classified {
                file: file.clone(),
                change: change("/", Class::Breaking, "the file was removed"),
            });
            if !allowed {
                outcome.problems.push(removed(file, contract));
            }
            continue;
        };
        let changes = file_changes(file, old, new);
        let breaking = changes.iter().any(|c| c.class == Class::Breaking);
        if breaking {
            match contract {
                Contract::CoreSettings => broken_settings = true,
                _ => outcome.problems.push(broken(file, contract)),
            }
        }
        outcome
            .changes
            .extend(changes.into_iter().map(|change| Classified {
                file: file.clone(),
                change,
            }));
    }
    for file in head.keys().filter(|f| !base.contains_key(*f)) {
        if contract(file).is_some() {
            outcome.changes.push(Classified {
                file: file.clone(),
                change: change("/", Class::Additive, "a new file"),
            });
        }
    }
    settings_rule(base, head, broken_settings, facts, &mut outcome.problems);
}

fn removed(file: &str, contract: Contract) -> String {
    match contract {
        Contract::Public | Contract::Errors => format!(
            "{file} was removed: clients may still use it. Keep it; a replacement goes under a new version (`v2.`)."
        ),
        Contract::Event => {
            format!("{file} was removed: old events of it can still be read back. Keep the file.")
        }
        Contract::Module => {
            format!("{file} was removed without bumping the manifest `api` version.")
        }
        Contract::CoreSettings | Contract::ModuleSettings => {
            format!("{file} was removed: settings files may still use it.")
        }
    }
}

fn broken(file: &str, contract: Contract) -> String {
    match contract {
        Contract::Public => {
            let name = file
                .rsplit('/')
                .next()
                .unwrap_or(file)
                .trim_end_matches(".json");
            let v2 = name
                .strip_prefix("v1.")
                .map_or_else(|| "a new version".to_owned(), |rest| format!("`v2.{rest}`"));
            format!(
                "`{name}` changed in a way that breaks clients. Add {v2} and keep `{name}` as it was."
            )
        }
        Contract::Errors => format!("{file} changed in a way that breaks clients."),
        Contract::Event => {
            let name = file.rsplit('/').next().unwrap_or(file);
            match parse_event_file(name) {
                Some((event_type, version)) => format!(
                    "`{event_type}` v{version} changed in a way that breaks reading old events. Add v{} with an upcaster from v{version}, and keep {} as the base branch has it (`git checkout <base> -- {API_DIR}/{}` if `cargo xtask schemas` rewrote it).",
                    version + 1,
                    event_file(event_type, version),
                    event_file(event_type, version)
                ),
                None => format!("{file} changed in a way that breaks reading old events."),
            }
        }
        Contract::ModuleSettings => {
            format!("{file} changed in a way that makes existing settings invalid.")
        }
        Contract::Module => format!(
            "{file} changed in a way that breaks existing modules. Bump the manifest `api` version."
        ),
        // Core settings follow `settings_rule` instead; this isn't reached.
        Contract::CoreSettings => format!("{file} changed in a way that breaks settings files."),
    }
}

/// (c): a breaking settings change, or any version bump, needs the bump,
/// migrations from the old version on, and fixtures of the old version.
fn settings_rule(
    base: &BTreeMap<String, Value>,
    head: &BTreeMap<String, Value>,
    broken: bool,
    facts: &Facts,
    problems: &mut Vec<String>,
) {
    let (Some(old), Some(new)) = (settings_version(base), settings_version(head)) else {
        if broken {
            problems.push(
                "settings/core.json changed in a way that breaks existing settings files, and has no `x-settings-version` to bump.".to_owned(),
            );
        }
        return;
    };
    if new < old {
        problems.push(format!("`settings_version` went down from {old} to {new}."));
        return;
    }
    if new == old {
        if broken {
            problems.push(format!(
                "settings/core.json changed in a way that breaks existing settings files. Bump `settings_version` (now {old}), register a migration from {old}, and add old files under {SETTINGS_FIXTURES}/v{old}/."
            ));
        }
        return;
    }
    for from in old..new {
        let Ok(from) = u32::try_from(from) else {
            continue;
        };
        if !facts.migrations.contains(&from) {
            problems.push(format!(
                "`settings_version` went from {old} to {new}, but no migration from {from} is registered."
            ));
        }
    }
    let has_fixture = u32::try_from(old)
        .ok()
        .and_then(|v| facts.settings_fixtures.get(&v))
        .is_some_and(|files| !files.is_empty());
    if !has_fixture {
        problems.push(format!(
            "`settings_version` went from {old} to {new}: add old settings files under {SETTINGS_FIXTURES}/v{old}/."
        ));
    }
}

/// (b) and the fixture rule: every older version of an event type is kept,
/// has an upcaster to the next, and a fixture that upcasts to a payload
/// valid as the current version.
fn old_events(head: &BTreeMap<String, Value>, facts: &Facts, problems: &mut Vec<String>) {
    let mut versions: BTreeMap<&str, BTreeSet<u16>> = BTreeMap::new();
    for file in head.keys() {
        if let Some((event_type, version)) = file.strip_prefix("events/").and_then(parse_event_file)
        {
            versions.entry(event_type).or_default().insert(version);
        }
    }
    for (event_type, known) in versions {
        let Some(&current) = known.last() else {
            continue;
        };
        for version in 1..current {
            if !known.contains(&version) {
                problems.push(format!(
                    "`{event_type}` v{version} has no schema file; keep {}.",
                    event_file(event_type, version)
                ));
            }
            if !facts.upcasters.contains(&(event_type.to_owned(), version)) {
                problems.push(format!(
                    "`{event_type}` has v{current} but no upcaster from v{version}."
                ));
            }
            let Some(fixture) = facts.event_fixtures.get(&(event_type.to_owned(), version)) else {
                problems.push(format!(
                    "`{event_type}` has v{current}: add a v{version} payload as {EVENT_FIXTURES}/{event_type}/v{version}.json."
                ));
                continue;
            };
            if let Some(schema) = head.get(&event_file(event_type, version))
                && let Err(e) = fits(schema, fixture)
            {
                problems.push(format!(
                    "The `{event_type}` v{version} fixture isn't a v{version} payload: {e}"
                ));
            }
            let lifted =
                (facts.upcast)(event_type, version, fixture.clone()).and_then(|(to, payload)| {
                    if to == current {
                        (facts.validate_event)(event_type, current, &payload)
                    } else {
                        Err(format!("it reached v{to}, not v{current}"))
                    }
                });
            if let Err(e) = lifted {
                problems.push(format!(
                    "The `{event_type}` v{version} fixture doesn't upcast to a valid v{current} payload: {e}"
                ));
            }
        }
    }
}

/// Whether `value` is valid against `schema`.
fn fits(schema: &Value, value: &Value) -> Result<(), String> {
    let validator = jsonschema::validator_for(schema).map_err(|e| e.to_string())?;
    validator.validate(value).map_err(|e| e.to_string())
}

/// Where this branch left `git_ref` (their merge base), so changes made on
/// the base since don't count as this branch's; `git_ref` itself when git
/// can't tell.
pub fn fork_point(root: &Path, git_ref: &str) -> String {
    crate::cmd::git(root)
        .args(["merge-base", git_ref, "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| git_ref.to_owned())
}

/// The schema files under [`API_DIR`] and [`API_JS`] at a git ref, or
/// `None` when it has none.
pub fn files_at(root: &Path, git_ref: &str) -> Result<Option<Files>> {
    let list = crate::cmd::git(root)
        .args([
            "ls-tree",
            "-r",
            "--name-only",
            git_ref,
            "--",
            API_DIR,
            API_JS,
        ])
        .output()
        .context("starting git")?;
    if !list.status.success() {
        anyhow::bail!(
            "git can't read {git_ref}: {}",
            String::from_utf8_lossy(&list.stderr).trim()
        );
    }
    let mut files = Files::new();
    for path in String::from_utf8_lossy(&list.stdout).lines() {
        let show = crate::cmd::git(root)
            .args(["show", &format!("{git_ref}:{path}")])
            .output()
            .context("starting git")?;
        if !show.status.success() {
            anyhow::bail!("git can't show {path} at {git_ref}");
        }
        files.insert(
            path.to_owned(),
            String::from_utf8_lossy(&show.stdout).into_owned(),
        );
    }
    Ok((files.keys().any(|p| p.starts_with(API_DIR))).then_some(files))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::schemas::{generate, pretty};

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the workspace")
            .to_path_buf()
    }

    /// Facts for a tree where nothing has a second version.
    fn facts() -> Facts {
        Facts {
            upcasters: BTreeSet::new(),
            migrations: BTreeSet::new(),
            event_fixtures: BTreeMap::new(),
            settings_fixtures: BTreeMap::new(),
            upcast: Box::new(|_, v, p| Ok((v, p))),
            validate_event: Box::new(|_, _, _| Ok(())),
            check_settings: Box::new(|_| Ok(())),
        }
    }

    /// The real tree as base, and a copy changed by `edit` as the new,
    /// committed state.
    fn scenario(edit: impl FnOnce(&mut Files)) -> (Files, Files) {
        let base = generate(&root()).expect("generated");
        let mut head = base.clone();
        edit(&mut head);
        (base, head)
    }

    fn edit_json(files: &mut Files, rel: &str, change: impl FnOnce(&mut Value)) {
        let path = format!("{API_DIR}/{rel}");
        let mut value: Value = serde_json::from_str(&files[&path]).expect("JSON");
        change(&mut value);
        files.insert(path, pretty(&value));
    }

    fn rename_in_event(files: &mut Files, rel: &str) {
        edit_json(files, rel, |doc| {
            let props = doc["properties"].as_object_mut().expect("properties");
            let key = props.remove("key").expect("a key field");
            props.insert("setting".into(), key);
            let required = doc["required"].as_array_mut().expect("required");
            for name in required.iter_mut() {
                if name == "key" {
                    *name = json!("setting");
                }
            }
        });
    }

    fn run(base: &Files, head: &Files, facts: &Facts) -> Outcome {
        check(head, head, Some(base), facts)
    }

    // covers: QA-08
    // suite: integration
    // acceptance: 0.9
    #[test]
    fn check_fails_when_snapshot_not_updated() {
        let (base, mut committed) = scenario(|_| {});
        let generated = base.clone();
        // Someone changed a type but not the committed schema.
        edit_json(&mut committed, "public/v1/v1.system.status.json", |doc| {
            doc["$defs"]["result"]["properties"]["extra"] = json!({"type": "string"});
        });
        committed.remove(&format!("{API_DIR}/INDEX.md"));
        let outcome = check(&generated, &committed, Some(&base), &facts());
        assert!(!outcome.passed());
        assert_eq!(
            outcome.stale,
            [
                format!("{API_DIR}/INDEX.md is missing"),
                format!("{API_DIR}/public/v1/v1.system.status.json is out of date"),
            ]
        );
        assert!(outcome.report().contains("run `cargo xtask schemas`"));

        // Up to date and unchanged: passes, and says so.
        let same = check(&generated, &generated, Some(&base), &facts());
        assert!(same.passed(), "{:?}", same.problems);
        assert!(same.report().contains("No schema changes."));
        assert!(
            check(&generated, &generated, None, &facts())
                .report()
                .contains("Not compared")
        );
    }

    // covers: QA-08
    // suite: integration
    // acceptance: 0.9
    #[test]
    fn check_fails_on_new_required_param_in_v1_method() {
        let (base, head) = scenario(|files| {
            edit_json(files, "public/v1/v1.settings.get.json", |doc| {
                let params = &mut doc["$defs"]["params"];
                params["properties"]["scope"] = json!({"type": "string"});
                params["required"] = json!(["scope"]);
            });
        });
        let outcome = run(&base, &head, &facts());
        assert!(!outcome.passed());
        assert!(
            outcome.problems[0].contains("Add `v2.settings.get` and keep `v1.settings.get`"),
            "{:?}",
            outcome.problems
        );
        assert!(outcome.report().contains("| `public/v1/v1.settings.get.json` | `params/scope` | `scope` was added as required | **breaking** |"), "{}", outcome.report());

        // An optional param, a new result field and a new method are fine.
        let (base, head) = scenario(|files| {
            edit_json(files, "public/v1/v1.settings.get.json", |doc| {
                doc["$defs"]["params"]["properties"]["scope"] = json!({"type": "string"});
                doc["$defs"]["result"]["properties"]["more"] = json!({"type": "string"});
                doc["$defs"]["result"]["required"] = json!(["value", "more"]);
            });
            let status = files[&format!("{API_DIR}/public/v1/v1.system.status.json")].clone();
            files.insert(format!("{API_DIR}/public/v1/v2.system.status.json"), status);
        });
        let outcome = run(&base, &head, &facts());
        assert!(outcome.passed(), "{:?}", outcome.problems);
        assert_eq!(outcome.changes.len(), 3, "{:?}", outcome.changes);

        // A removed method breaks clients.
        let (base, head) = scenario(|files| {
            files.remove(&format!("{API_DIR}/public/v1/v1.system.status.json"));
        });
        let outcome = run(&base, &head, &facts());
        assert!(
            outcome.problems[0].contains("was removed"),
            "{:?}",
            outcome.problems
        );
    }

    // covers: QA-08
    // suite: integration
    // acceptance: 0.9
    #[test]
    fn check_fails_on_event_field_rename_without_new_version() {
        let (base, head) =
            scenario(|files| rename_in_event(files, "events/setting.changed.v1.json"));
        let outcome = run(&base, &head, &facts());
        assert!(!outcome.passed());
        assert_eq!(
            outcome.problems,
            [
                "`setting.changed` v1 changed in a way that breaks reading old events. Add v2 with an upcaster from v1, and keep events/setting.changed.v1.json as the base branch has it (`git checkout <base> -- docs/claude/api/events/setting.changed.v1.json` if `cargo xtask schemas` rewrote it)."
            ]
        );
    }

    /// The base, and a head where `setting.changed` v2 renames `key` to
    /// `setting` next to the kept v1 file.
    fn second_event_version() -> (Files, Files) {
        scenario(|files| {
            let v1 = format!("{API_DIR}/events/setting.changed.v1.json");
            let v2 = format!("{API_DIR}/events/setting.changed.v2.json");
            files.insert(v2, files[&v1].clone());
            rename_in_event(files, "events/setting.changed.v2.json");
        })
    }

    /// A synthetic v1 → v2 upcaster renaming `key` to `setting`, and a
    /// validator that wants `setting`.
    fn upcasting_facts() -> Facts {
        let mut facts = facts();
        facts.upcasters.insert(("setting.changed".into(), 1));
        facts.event_fixtures.insert(
            ("setting.changed".into(), 1),
            json!({"key": "ui.theme", "old": "dark", "new": "light", "source": "user"}),
        );
        facts.upcast = Box::new(|event_type, version, mut payload| {
            assert_eq!((event_type, version), ("setting.changed", 1));
            let key = payload
                .as_object_mut()
                .and_then(|p| p.remove("key"))
                .ok_or("no key")?;
            payload["setting"] = key;
            Ok((2, payload))
        });
        facts.validate_event = Box::new(|_, version, payload| {
            if version == 2 && payload.get("setting").is_some() && payload.get("key").is_none() {
                Ok(())
            } else {
                Err("not a v2 payload".into())
            }
        });
        facts
    }

    // covers: QA-08
    // suite: integration
    // acceptance: 0.9
    #[test]
    fn check_passes_with_new_event_version_and_upcaster() {
        let (base, head) = second_event_version();
        let outcome = run(&base, &head, &upcasting_facts());
        assert!(outcome.passed(), "{:?}", outcome.problems);
        assert!(outcome.changes.iter().any(
            |c| c.file == "events/setting.changed.v2.json" && c.change.class == Class::Additive
        ));

        // Without the upcaster or the fixture it fails, saying which.
        let outcome = run(&base, &head, &facts());
        assert_eq!(
            outcome.problems,
            [
                "`setting.changed` has v2 but no upcaster from v1.",
                "`setting.changed` has v2: add a v1 payload as crates/priya-api/tests/fixtures/events/setting.changed/v1.json.",
            ]
        );
        // And without the old file.
        let mut no_v1 = head.clone();
        no_v1.remove(&format!("{API_DIR}/events/setting.changed.v1.json"));
        let outcome = run(&base, &no_v1, &upcasting_facts());
        assert!(
            outcome.problems.iter().any(|p| p.contains("was removed")),
            "{:?}",
            outcome.problems
        );
        assert!(
            outcome
                .problems
                .iter()
                .any(|p| p.contains("has no schema file"))
        );
    }

    // covers: QA-08
    // suite: unit
    // acceptance: 0.9
    #[test]
    fn upcast_chain_produces_valid_current_payload() {
        let (_, head) = second_event_version();
        let head = schemas(&head);
        let mut problems = Vec::new();
        old_events(&head, &upcasting_facts(), &mut problems);
        assert_eq!(problems, Vec::<String>::new());

        // An upcaster that stops short, or lifts to an invalid payload.
        let mut short = upcasting_facts();
        short.upcast = Box::new(|_, v, p| Ok((v, p)));
        old_events(&head, &short, &mut problems);
        assert_eq!(
            problems,
            [
                "The `setting.changed` v1 fixture doesn't upcast to a valid v2 payload: it reached v1, not v2"
            ]
        );
        problems.clear();
        // A fixture that isn't really the old version.
        let mut newer = upcasting_facts();
        newer.event_fixtures.insert(
            ("setting.changed".into(), 1),
            json!({"setting": "ui.theme", "source": "user"}),
        );
        newer.upcast = Box::new(|_, _, p| Ok((2, p)));
        old_events(&head, &newer, &mut problems);
        assert!(
            problems[0].starts_with("The `setting.changed` v1 fixture isn't a v1 payload: "),
            "{problems:?}"
        );
        problems.clear();
        let mut invalid = upcasting_facts();
        invalid.upcast = Box::new(|_, _, p| Ok((2, p)));
        old_events(&head, &invalid, &mut problems);
        assert!(problems[0].ends_with("not a v2 payload"), "{problems:?}");
    }

    fn remove_settings_key(files: &mut Files) {
        edit_json(files, "settings/core.json", |doc| {
            let ui = doc["$defs"]["Ui"]["properties"]
                .as_object_mut()
                .expect("Ui properties");
            ui.remove("reduced_motion").expect("reduced_motion");
        });
    }

    // covers: QA-08
    // suite: integration
    // acceptance: 0.9
    #[test]
    fn check_fails_on_removed_settings_key_without_migration() {
        let (base, head) = scenario(remove_settings_key);
        let outcome = run(&base, &head, &facts());
        assert!(!outcome.passed());
        assert_eq!(
            outcome.problems,
            [
                "settings/core.json changed in a way that breaks existing settings files. Bump `settings_version` (now 1), register a migration from 1, and add old files under crates/priya-core/tests/fixtures/settings/v1/."
            ]
        );

        // Bumped, but with no migration and no old file.
        let (base, head) = scenario(|files| {
            remove_settings_key(files);
            edit_json(files, "settings/core.json", |doc| {
                doc["x-settings-version"] = json!(2)
            });
        });
        let outcome = run(&base, &head, &facts());
        assert_eq!(
            outcome.problems,
            [
                "`settings_version` went from 1 to 2, but no migration from 1 is registered.",
                "`settings_version` went from 1 to 2: add old settings files under crates/priya-core/tests/fixtures/settings/v1/.",
            ]
        );
    }

    // covers: QA-08
    // suite: integration
    // acceptance: 0.9
    #[test]
    fn check_passes_with_migration_and_fixture() {
        let (base, head) = scenario(|files| {
            remove_settings_key(files);
            edit_json(files, "settings/core.json", |doc| {
                doc["x-settings-version"] = json!(2)
            });
        });
        let mut facts = facts();
        facts.migrations.insert(1);
        facts.settings_fixtures.insert(
            1,
            vec![("old.toml".into(), "[ui]\nreduced_motion = true\n".into())],
        );
        facts.check_settings = Box::new(|text| {
            if text.contains("reduced_motion") {
                Err("unknown setting `ui.reduced_motion`".into())
            } else {
                Ok(())
            }
        });
        // The fixture must also migrate to valid settings.
        let outcome = run(&base, &head, &facts);
        assert_eq!(
            outcome.problems,
            [
                "The old settings file crates/priya-core/tests/fixtures/settings/v1/old.toml doesn't migrate to valid settings: unknown setting `ui.reduced_motion`"
            ]
        );
        facts.check_settings = Box::new(|_| Ok(()));
        let outcome = run(&base, &head, &facts);
        assert!(outcome.passed(), "{:?}", outcome.problems);
        assert!(
            outcome
                .changes
                .iter()
                .any(|c| c.change.class == Class::Breaking)
        );
    }

    // covers: QA-08
    // suite: unit
    // acceptance: 0.9
    #[test]
    fn manifest_bumps_errors_and_odd_files_are_classified() {
        // The protocol changes in place: refused. With api v2: fine.
        let (base, head) = scenario(|files| {
            edit_json(files, "module/host-v1/initialize.json", |doc| {
                // priyad would now refuse what today's modules answer.
                let result = &mut doc["$defs"]["result"];
                result["properties"]["extra"] = json!({"type": "string"});
                result["required"]
                    .as_array_mut()
                    .expect("required")
                    .push(json!("extra"));
            });
        });
        let outcome = run(&base, &head, &facts());
        assert!(
            outcome.problems[0].contains("Bump the manifest `api` version"),
            "{:?}",
            outcome.problems
        );
        let (base, head) = scenario(|files| {
            let moved: Vec<String> = files
                .keys()
                .filter(|p| p.starts_with(&format!("{API_DIR}/module/")))
                .cloned()
                .collect();
            for path in moved {
                let text = files.remove(&path).expect("present");
                let path = path
                    .replace("manifest.v1", "manifest.v2")
                    .replace("host-v1", "host-v2");
                files.insert(path, text);
            }
        });
        let outcome = run(&base, &head, &facts());
        assert!(outcome.passed(), "{:?}", outcome.problems);

        // Error kinds: a new one is additive, a changed code breaks.
        let (base, head) = scenario(|files| {
            edit_json(files, "public/v1/errors.json", |doc| {
                doc["x-codes"]["new_kind"] = json!(-32100);
            });
        });
        assert!(run(&base, &head, &facts()).passed());
        let (base, head) = scenario(|files| {
            edit_json(files, "public/v1/errors.json", |doc| {
                doc["x-codes"]["timeout"] = json!(-32100);
            });
        });
        let outcome = run(&base, &head, &facts());
        assert!(
            outcome
                .changes
                .iter()
                .any(|c| c.change.what.contains("code of `timeout`"))
        );
        assert!(!outcome.passed());

        // A module's own settings can't break; the version can't go down.
        let mut base = generate(&root()).expect("generated");
        let schema = json!({"type": "object", "properties": {"a": {"type": "string"}}});
        base.insert(
            format!("{API_DIR}/settings/modules/m.json"),
            pretty(&schema),
        );
        let mut head = base.clone();
        edit_json(&mut head, "settings/modules/m.json", |doc| {
            doc["properties"]["a"]["type"] = json!("integer");
        });
        edit_json(&mut head, "settings/core.json", |doc| {
            doc["x-settings-version"] = json!(0)
        });
        let outcome = run(&base, &head, &facts());
        assert_eq!(outcome.problems.len(), 2, "{:?}", outcome.problems);
        assert!(outcome.problems.iter().any(|p| p.contains("went down")));
        assert_eq!(contract("INDEX.md"), None);

        // A method that stops being read-only breaks modules calling it.
        let (base, head) = scenario(|files| {
            edit_json(files, "public/v1/v1.system.status.json", |doc| {
                doc["x-read-only"] = json!(false);
            });
            edit_json(files, "public/v1/v1.settings.set.json", |doc| {
                doc["x-read-only"] = json!(true);
                doc["x-notification"] = json!("v1.settings.changed");
            });
        });
        let outcome = run(&base, &head, &facts());
        assert_eq!(outcome.problems.len(), 1, "{:?}", outcome.problems);
        assert!(outcome.problems[0].contains("`v1.system.status` changed"));
        assert_eq!(outcome.changes.len(), 3, "{:?}", outcome.changes);
        // A breaking settings change with no version to bump.
        let mut problems = Vec::new();
        settings_rule(
            &BTreeMap::new(),
            &BTreeMap::new(),
            true,
            &facts(),
            &mut problems,
        );
        assert!(
            problems[0].contains("no `x-settings-version`"),
            "{problems:?}"
        );
        assert_eq!(module_version("module/host-v3/ping.json"), Some(3));
    }

    // covers: QA-08
    // suite: integration
    // acceptance: 0.9
    #[test]
    fn base_schemas_are_read_from_git_and_the_command_checks_against_them() {
        // The whole command on an empty folder: nothing committed yet is
        // stale; once written, it passes.
        let dir = tempfile::tempdir().expect("a folder");
        let stale = crate::schemas::run(dir.path(), true, None).expect_err("nothing written yet");
        assert!(stale.to_string().contains("run `cargo xtask schemas`"));
        let report = dir.path().join(crate::schemas::REPORT);
        assert!(
            fs::read_to_string(&report)
                .expect("a report")
                .contains("is missing")
        );
        crate::schemas::run(dir.path(), false, None).expect("written");
        crate::schemas::run(dir.path(), true, None).expect("up to date");
        assert!(
            fs::read_to_string(&report)
                .expect("a report")
                .contains("Not compared")
        );

        // A base is read from git, where there is a repository.
        let root = root();
        let Ok(Some(head)) = files_at(&root, "HEAD") else {
            return; // no git here (a source package)
        };
        assert!(head.contains_key(&format!("{API_DIR}/INDEX.md")));
        assert!(head.contains_key(API_JS));
        assert!(files_at(&root, "no-such-ref-anywhere").is_err());
        assert_eq!(
            fork_point(&root, "no-such-ref-anywhere"),
            "no-such-ref-anywhere"
        );
        assert_eq!(fork_point(&root, "HEAD").len(), 40);
    }

    // covers: QA-08
    // suite: integration
    // acceptance: 0.9
    #[test]
    fn the_real_tree_reads_its_facts_and_passes_against_itself() {
        let root = root();
        let facts = Facts::real(&root).expect("facts");
        let generated = generate(&root).expect("generated");
        let outcome = check(&generated, &generated, Some(&generated), &facts);
        assert!(outcome.passed(), "{:?}", outcome.problems);
        assert!((facts.check_settings)("[ui]\ntheme = \"light\"\n").is_ok());
        assert!((facts.check_settings)("[ui]\ntheme = 5\n").is_err());
        assert!((facts.validate_event)("nope.nothing", 1, &json!({})).is_err());

        // Fixtures are read from their folders.
        let dir = tempfile::tempdir().expect("a folder");
        let events = dir.path().join(EVENT_FIXTURES).join("setting.changed");
        let settings = dir.path().join(SETTINGS_FIXTURES).join("v1");
        fs::create_dir_all(&events).expect("folders");
        fs::create_dir_all(&settings).expect("folders");
        fs::write(events.join("v1.json"), "{\"key\": \"ui.theme\"}").expect("written");
        fs::write(events.join("notes.md"), "how it was made").expect("written");
        // Notes next to the type folders are fine too.
        fs::write(events.join("../README.md"), "how they were made").expect("written");
        fs::write(events.join("../.DS_Store"), "junk").expect("written");
        fs::write(settings.join("old.toml"), "[ui]\n").expect("written");
        fs::write(settings.join("RECORDED.md"), "how it was made").expect("written");
        let read = Facts::real(dir.path()).expect("facts");
        assert_eq!(
            read.event_fixtures.keys().collect::<Vec<_>>(),
            [&("setting.changed".to_owned(), 1)]
        );
        assert_eq!(
            read.settings_fixtures[&1],
            [("old.toml".to_owned(), "[ui]\n".to_owned())]
        );
    }
}
