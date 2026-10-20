//! `cargo xtask schemas`: writes the JSON Schemas of everything outside
//! code relies on (the public API, events, settings, the module manifest
//! and protocol) under `docs/claude/api/`, with an index and the UI's
//! generated helpers (sub-phase 0.9).
//!
//! The output is deterministic: keys sorted, two-space indents, a stable
//! `$id` per file. An old event version that is no longer registered (its
//! struct is gone) can't be generated: its committed file is kept as it is.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::schema_check;
use priya_api::events::REGISTRY;
use priya_api::methods::{ALL, MethodInfo, NOTIFICATIONS};
use priya_api::module::manifest::{API_VERSION, ModuleManifest};
use priya_api::module::protocol::{HOST_METHODS, MODULE_METHODS};
use priya_core::settings::{CURRENT_VERSION, Settings};
use serde_json::{Map, Value, json};

/// Where the schemas live.
pub const API_DIR: &str = "docs/claude/api";
/// The UI's helpers, made from the public schemas.
pub const API_JS: &str = "ui/PriyaUI/api/Api.js";
/// Where first-party modules live; the examples aren't shipped.
const MODULES_DIR: &str = "modules";
/// The folder of examples under [`MODULES_DIR`], which aren't shipped.
const EXAMPLES: &str = "examples";
const DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";
/// Each file's `$id` is where it is published.
const ID_BASE: &str = "https://github.com/prrockzed/PriyaOS/blob/main/docs/claude/api/";

/// Generated files: repository-relative path → contents.
pub type Files = BTreeMap<String, String>;

/// Where `--check` writes its PR report section.
pub const REPORT: &str = "target/schemas/report.md";

/// Writes the schemas, or with `check` compares them with the committed
/// ones and, given a `base` ref, classifies the changes since it.
pub fn run(root: &Path, check: bool, base: Option<&str>) -> Result<()> {
    let files = generate(root)?;
    if !check {
        write_all(root, &files)?;
        println!(
            "Wrote {} files under {API_DIR}, and {API_JS}.",
            files.len() - 1
        );
        return Ok(());
    }
    let committed = on_disk(root)?;
    let base = match base {
        Some(git_ref) => schema_check::files_at(root, &schema_check::fork_point(root, git_ref))?,
        None => None,
    };
    let facts = schema_check::Facts::real(root)?;
    let outcome = schema_check::check(&files, &committed, base.as_ref(), &facts);
    let report = outcome.report();
    let path = root.join(REPORT);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, &report).with_context(|| format!("writing {REPORT}"))?;
    println!("{report}");
    if !outcome.stale.is_empty() {
        bail!("the schema files are out of date; run `cargo xtask schemas` and commit");
    }
    if !outcome.passed() {
        bail!("a schema change breaks compatibility (see above, or {REPORT})");
    }
    println!("The {} schema files match the code.", files.len());
    Ok(())
}

/// Every generated file, from the code and the committed old event
/// versions in `root`.
pub fn generate(root: &Path) -> Result<Files> {
    let mut docs = Docs::default();
    for method in ALL {
        docs.method("public/v1", method, Direction::Public)?;
    }
    for notification in NOTIFICATIONS {
        let schema = bundle(&[("params", (notification.params)())])?;
        docs.add(
            &format!("public/v1/{}.json", notification.name),
            notification.name,
            "A notification the daemon sends.",
            [
                ("x-kind", json!("notification")),
                ("x-direction", json!("daemon_to_client")),
            ],
            schema,
        );
    }
    docs.errors();
    for kind in REGISTRY {
        docs.add(
            &event_file(kind.event_type, kind.version),
            &format!("{} v{}", kind.event_type, kind.version),
            "An event payload.",
            [
                ("x-event-type", json!(kind.event_type)),
                ("x-event-version", json!(kind.version)),
            ],
            schema_value(kind.schema()),
        );
    }
    let mut files = docs.files;
    keep_old_events(root, &mut files)?;

    let mut docs = Docs { files };
    docs.add(
        "settings/core.json",
        "Settings",
        "Every core setting, as in settings.toml.",
        [("x-settings-version", json!(CURRENT_VERSION))],
        schema_value(Settings::json_schema()),
    );
    for (name, schema) in module_settings(root)? {
        docs.add(
            &format!("settings/modules/{name}.json"),
            &format!("modules.{name}"),
            "A first-party module's settings.",
            [],
            schema,
        );
    }
    docs.add(
        &format!("module/manifest.{API_VERSION}.json"),
        "module.toml",
        "A module's manifest.",
        [("x-api-version", json!(API_VERSION))],
        schema_value(schemars::schema_for!(ModuleManifest)),
    );
    let host_dir = format!("module/host-{API_VERSION}");
    for method in HOST_METHODS {
        docs.method(&host_dir, method, Direction::ModuleToHost)?;
    }
    for method in MODULE_METHODS {
        docs.method(&host_dir, method, Direction::HostToModule)?;
    }

    let mut files: Files = docs
        .files
        .into_iter()
        .map(|(path, text)| (format!("{API_DIR}/{path}"), text))
        .collect();
    let index = index(&files)?;
    files.insert(format!("{API_DIR}/INDEX.md"), index);
    files.insert(API_JS.to_owned(), api_js(&files)?);
    Ok(files)
}

/// `events/<type>.v<N>.json`, relative to [`API_DIR`].
pub fn event_file(event_type: &str, version: u16) -> String {
    format!("events/{event_type}.v{version}.json")
}

/// Who calls a method, which decides which way its params and result flow.
#[derive(Debug, Clone, Copy)]
enum Direction {
    /// A client calls priyad.
    Public,
    /// A module calls priyad (`host.*`).
    ModuleToHost,
    /// priyad calls a module.
    HostToModule,
}

impl Direction {
    fn name(self) -> &'static str {
        match self {
            Self::Public => "client_to_daemon",
            Self::ModuleToHost => "module_to_host",
            Self::HostToModule => "host_to_module",
        }
    }
}

/// Generated files relative to [`API_DIR`].
#[derive(Default)]
struct Docs {
    files: Files,
}

impl Docs {
    fn add<const N: usize>(
        &mut self,
        path: &str,
        title: &str,
        description: &str,
        extra: [(&str, Value); N],
        schema: Value,
    ) {
        let mut doc = match schema {
            Value::Object(map) => map,
            other => {
                let mut map = Map::new();
                map.insert("allOf".into(), json!([other]));
                map
            }
        };
        doc.insert("$schema".into(), json!(DRAFT));
        doc.insert("$id".into(), json!(format!("{ID_BASE}{path}")));
        doc.insert("title".into(), json!(title));
        doc.entry("description")
            .or_insert_with(|| json!(description));
        for (key, value) in extra {
            doc.insert(key.into(), value);
        }
        self.files
            .insert(path.to_owned(), pretty(&Value::Object(doc)));
    }

    fn method(&mut self, dir: &str, method: &MethodInfo, direction: Direction) -> Result<()> {
        let schema = bundle(&[("params", (method.params)()), ("result", (method.result)())])
            .with_context(|| method.name.to_owned())?;
        let mut extra = vec![
            ("x-kind", json!("method")),
            ("x-direction", json!(direction.name())),
            ("x-read-only", json!(method.read_only)),
        ];
        if let Some(notification) = method.notification {
            extra.push(("x-notification", json!(notification)));
        }
        let mut doc = schema;
        if let Value::Object(map) = &mut doc {
            for (key, value) in extra {
                map.insert(key.into(), value);
            }
        }
        self.add(
            &format!("{dir}/{}.json", method.name),
            method.name,
            method.summary,
            [],
            doc,
        );
        Ok(())
    }

    /// `errors.json`: the `data` of every error, and the code of each kind.
    fn errors(&mut self) {
        let codes: Map<String, Value> = priya_api::rpc::KINDS
            .iter()
            .map(|kind| {
                (
                    kind.as_str().to_owned(),
                    json!(priya_api::rpc::code_for(*kind)),
                )
            })
            .collect();
        self.add(
            "public/v1/errors.json",
            "Errors",
            "The `data` of every error reply: clients branch on `kind`, never on the message. \
             `x-codes` gives the JSON-RPC code of each kind.",
            [("x-codes", Value::Object(codes))],
            {
                let mut schema = schema_value(schemars::schema_for!(priya_api::error::ApiError));
                // This file's own description says what `x-codes` is.
                if let Value::Object(map) = &mut schema {
                    map.remove("description");
                }
                schema
            },
        );
    }
}

/// One document holding several root schemas under `$defs` (`params`,
/// `result`…), with the types they share defined once.
fn bundle(parts: &[(&str, schemars::Schema)]) -> Result<Value> {
    bundle_values(
        parts
            .iter()
            .map(|(name, schema)| (*name, schema_value(schema.clone())))
            .collect(),
    )
}

fn bundle_values(parts: Vec<(&str, Value)>) -> Result<Value> {
    let mut defs = Map::new();
    let mut roots = Vec::new();
    for (name, mut value) in parts {
        // A type that contains itself points at its root, `#`, which is
        // `$defs/<name>` once bundled: in the root and in its own defs.
        retarget_root_refs(&mut value, &format!("#/$defs/{name}"));
        if let Value::Object(map) = &mut value {
            if let Some(Value::Object(inner)) = map.remove("$defs") {
                for (key, def) in inner {
                    match defs.get(&key) {
                        Some(existing) if existing != &def => {
                            bail!("two different types are both called `{key}`")
                        }
                        _ => {
                            defs.insert(key, def);
                        }
                    }
                }
            }
            map.remove("$schema");
        }
        roots.push((name, value));
    }
    for (name, value) in roots {
        if defs.insert(name.to_owned(), value).is_some() {
            bail!("a type is called `{name}`, which the bundle uses itself");
        }
    }
    Ok(json!({ "$defs": defs }))
}

fn retarget_root_refs(value: &mut Value, target: &str) {
    match value {
        Value::Object(map) => {
            if map.get("$ref") == Some(&json!("#")) {
                map.insert("$ref".into(), json!(target));
            }
            for child in map.values_mut() {
                retarget_root_refs(child, target);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| retarget_root_refs(v, target)),
        _ => {}
    }
}

fn schema_value(schema: schemars::Schema) -> Value {
    let mut value = schema.to_value();
    if let Value::Object(map) = &mut value {
        map.remove("$schema");
    }
    value
}

/// Committed event files older than the registered version of their type:
/// the frozen schemas of versions that can still be read back.
fn keep_old_events(root: &Path, files: &mut Files) -> Result<()> {
    let dir = root.join(API_DIR).join("events");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some((event_type, version)) = parse_event_file(name) else {
            continue;
        };
        let current = REGISTRY
            .iter()
            .filter(|k| k.event_type == event_type)
            .map(|k| k.version)
            .max();
        if current.is_some_and(|current| version < current) {
            let text =
                fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            // A version still registered is generated from its struct.
            files.entry(event_file(event_type, version)).or_insert(text);
        }
    }
    Ok(())
}

/// `setting.changed.v2.json` → `("setting.changed", 2)`.
pub fn parse_event_file(name: &str) -> Option<(&str, u16)> {
    let stem = name.strip_suffix(".json")?;
    let (event_type, version) = stem.rsplit_once(".v")?;
    Some((event_type, version.parse().ok()?))
}

/// The settings schemas of first-party modules (not the examples), by name.
fn module_settings(root: &Path) -> Result<Vec<(String, Value)>> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(root.join(MODULES_DIR)) else {
        return Ok(found);
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| !p.ends_with(EXAMPLES) && p.join("module.toml").is_file())
        .collect();
    dirs.sort();
    for dir in dirs {
        let manifest_path = dir.join("module.toml");
        let text = fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading {}", manifest_path.display()))?;
        let manifest = ModuleManifest::parse(&text)
            .map_err(|e| anyhow::anyhow!("{}: {e}", manifest_path.display()))?;
        let Some(file) = &manifest.provides.settings_schema else {
            continue;
        };
        let schema_path = dir.join(file);
        let schema: Value = serde_json::from_str(
            &fs::read_to_string(&schema_path)
                .with_context(|| format!("reading {}", schema_path.display()))?,
        )
        .with_context(|| format!("{} isn't JSON", schema_path.display()))?;
        found.push((manifest.module.name.clone(), schema));
    }
    Ok(found)
}

/// JSON with keys sorted and a final newline.
pub fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(&sorted(value)).unwrap_or_default();
    text.push('\n');
    text
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let ordered: BTreeMap<&String, Value> =
                map.iter().map(|(k, v)| (k, sorted(v))).collect();
            Value::Object(ordered.into_iter().map(|(k, v)| (k.clone(), v)).collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// The files under [`API_DIR`] (hidden ones aside) and [`API_JS`] as they
/// are in `root`.
pub fn on_disk(root: &Path) -> Result<Files> {
    let mut files = Files::new();
    let mut paths = committed(root)?;
    if root.join(API_JS).is_file() {
        paths.push(API_JS.to_owned());
    }
    for path in paths {
        let text =
            fs::read_to_string(root.join(&path)).with_context(|| format!("reading {path}"))?;
        files.insert(path, text);
    }
    Ok(files)
}

/// Every file under [`API_DIR`], repository-relative, except hidden ones
/// (such as Finder's `.DS_Store`).
fn committed(root: &Path) -> Result<Vec<String>> {
    let mut found = Vec::new();
    let mut pending = vec![root.join(API_DIR)];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let path = entry?.path();
            if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
            {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(rel) = path.strip_prefix(root) {
                found.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.sort();
    Ok(found)
}

fn write_all(root: &Path, files: &Files) -> Result<()> {
    // The folder is generated: files nothing generates any more go.
    for path in committed(root)? {
        if !files.contains_key(&path) {
            fs::remove_file(root.join(&path)).with_context(|| format!("removing {path}"))?;
            println!("Removed {path}, which nothing generates.");
        }
    }
    for (path, text) in files {
        let full = root.join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        if fs::read_to_string(&full).ok().as_deref() != Some(text) {
            fs::write(&full, text).with_context(|| format!("writing {path}"))?;
        }
    }
    Ok(())
}

/// A generated file parsed back.
fn parsed(files: &Files, path: &str) -> Result<Value> {
    serde_json::from_str(&files[path]).with_context(|| format!("{path} isn't JSON"))
}

/// The files under `dir` (relative to [`API_DIR`]), by their name.
fn under<'a>(files: &'a Files, dir: &str) -> impl Iterator<Item = (&'a str, &'a String)> {
    let prefix = format!("{API_DIR}/{dir}/");
    files.iter().filter_map(move |(path, text)| {
        path.strip_prefix(&prefix)
            .filter(|rest| !rest.contains('/'))
            .map(|rest| (rest, text))
    })
}

fn index(files: &Files) -> Result<String> {
    let mut out = String::from(
        "# API schemas\n\n\
         Generated by `cargo xtask schemas` from the code (sub-phase 0.9); don't edit.\n\
         `cargo xtask schemas --check` in CI fails when these files don't match the code, and\n\
         when a change breaks compatibility without its version bump\n\
         ([09-testing §6](../architecture/09-testing.md)).\n\n",
    );
    let line = |out: &mut String, link: &str, name: &str, about: &str| {
        let _ = writeln!(out, "| [`{name}`]({link}) | {about} |");
    };

    out.push_str("## Public API v1\n\nMethods (params and result under `$defs`) and notifications.\n\n| Name | What |\n|---|---|\n");
    for (name, text) in under(files, "public/v1") {
        let doc: Value = serde_json::from_str(text)?;
        let title = doc["title"].as_str().unwrap_or(name);
        let mut about = doc["description"].as_str().unwrap_or_default().to_owned();
        if doc["x-read-only"] == json!(true) {
            about.push_str(" Read-only.");
        }
        if let Some(notification) = doc["x-notification"].as_str() {
            let _ = write!(about, " Sends `{notification}`.");
        }
        line(&mut out, &format!("public/v1/{name}"), title, &about);
    }

    out.push_str("\n## Events\n\nPayloads, one file per type and version. Old versions stay, with an upcaster to the next.\n\n| Type | Version |\n|---|---|\n");
    for (name, text) in under(files, "events") {
        let doc: Value = serde_json::from_str(text)?;
        let event_type = doc["x-event-type"].as_str().unwrap_or(name);
        let version = &doc["x-event-version"];
        let _ = writeln!(out, "| [`{event_type}`](events/{name}) | {version} |");
    }

    out.push_str("\n## Settings\n\n| Section | What |\n|---|---|\n");
    let core = parsed(files, &format!("{API_DIR}/settings/core.json"))?;
    line(
        &mut out,
        "settings/core.json",
        "core",
        &format!(
            "Every core setting; format version {}.",
            core["x-settings-version"]
        ),
    );
    let modules: Vec<_> = under(files, "settings/modules").collect();
    for (name, _) in &modules {
        let module = name.trim_end_matches(".json");
        line(
            &mut out,
            &format!("settings/modules/{name}"),
            &format!("modules.{module}"),
            "A first-party module's settings.",
        );
    }
    if modules.is_empty() {
        out.push_str("\nNo first-party module has settings yet (the examples aren't shipped).\n");
    }

    let _ = write!(
        out,
        "\n## Modules\n\nThe manifest, and the protocol between priyad and a module (`x-direction` says who calls).\n\n| Name | What |\n|---|---|\n"
    );
    line(
        &mut out,
        &format!("module/manifest.{API_VERSION}.json"),
        "module.toml",
        &format!("A module's manifest, API {API_VERSION}."),
    );
    let host = format!("module/host-{API_VERSION}");
    for (name, text) in under(files, &host) {
        let doc: Value = serde_json::from_str(text)?;
        let title = doc["title"].as_str().unwrap_or(name);
        let direction = doc["x-direction"]
            .as_str()
            .unwrap_or_default()
            .replace('_', " ");
        let about = format!(
            "{} ({direction}).",
            doc["description"]
                .as_str()
                .unwrap_or_default()
                .trim_end_matches('.')
        );
        line(&mut out, &format!("{host}/{name}"), title, &about);
    }
    Ok(out)
}

/// The UI's helpers: method and notification names, the fields of each
/// result and notification, and the error kinds, as QML JavaScript.
fn api_js(files: &Files) -> Result<String> {
    let mut methods = Vec::new();
    let mut notifications = Vec::new();
    let mut fields = Vec::new();
    for (name, text) in under(files, "public/v1") {
        let doc: Value = serde_json::from_str(text)?;
        let title = doc["title"].as_str().unwrap_or_default().to_owned();
        let part = match doc["x-kind"].as_str() {
            Some("method") => {
                methods.push(title.clone());
                "result"
            }
            Some("notification") => {
                notifications.push(title.clone());
                "params"
            }
            _ if name == "errors.json" => continue,
            _ => bail!("{name} is neither a method nor a notification"),
        };
        fields.push((title, top_fields(&doc, part)));
    }
    let errors = parsed(files, &format!("{API_DIR}/public/v1/errors.json"))?;
    let kinds: Vec<String> = errors["x-codes"]
        .as_object()
        .map(|codes| codes.keys().cloned().collect())
        .unwrap_or_default();

    let mut out = String::from(
        ".pragma library\n\
         // Generated by `cargo xtask schemas` from the public API schemas in\n\
         // docs/claude/api (sub-phase 0.9), so the UI and priyad can't drift.\n\
         // Don't edit: change the Rust types and run the command again.\n\n",
    );
    names(&mut out, "methods", "Method names.", &methods)?;
    names(
        &mut out,
        "notifications",
        "Notification names.",
        &notifications,
    )?;
    out.push_str("// The top-level fields of each method's result and each notification's params.\nvar fields = Object.freeze({\n");
    for (name, list) in &fields {
        let quoted: Vec<String> = list.iter().map(|f| js_string(f)).collect();
        let _ = writeln!(
            out,
            "    {}: Object.freeze([{}]),",
            js_string(name),
            quoted.join(", ")
        );
    }
    out.push_str(
        "});\n\n// Error kinds, the `kind` of an error's data.\nvar errorKinds = Object.freeze([\n",
    );
    for kind in kinds {
        let _ = writeln!(out, "    {},", js_string(&kind));
    }
    out.push_str("]);\n");
    Ok(out)
}

/// A frozen object of `jsName: "wire.name"` pairs; two names that would
/// share a JS name are refused rather than one silently replacing the other.
fn names(out: &mut String, var: &str, about: &str, list: &[String]) -> Result<()> {
    let mut seen = BTreeMap::new();
    let _ = writeln!(out, "// {about}\nvar {var} = Object.freeze({{");
    for name in list {
        let js = js_name(name);
        if let Some(other) = seen.insert(js.clone(), name) {
            bail!("`{other}` and `{name}` would both be `{var}.{js}` in {API_JS}");
        }
        let _ = writeln!(out, "    {js}: {},", js_string(name));
    }
    out.push_str("});\n\n");
    Ok(())
}

/// A JS string literal (JSON's is one).
fn js_string(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}

/// `v1.system.status` → `systemStatus`; other versions keep theirs
/// (`v2.system.status` → `v2SystemStatus`).
fn js_name(method: &str) -> String {
    let rest = method.strip_prefix("v1.").unwrap_or(method);
    let mut out = String::new();
    for (i, word) in rest.split(['.', '_']).enumerate() {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            if i == 0 {
                out.push(first);
            } else {
                out.extend(first.to_uppercase());
            }
            out.extend(chars);
        }
    }
    out
}

/// The property names of `$defs/<part>`, following a `$ref`.
fn top_fields(doc: &Value, part: &str) -> Vec<String> {
    let mut node = &doc["$defs"][part];
    for _ in 0..8 {
        match node.get("$ref").and_then(Value::as_str) {
            Some(reference) => match reference.strip_prefix('#').and_then(|p| doc.pointer(p)) {
                Some(target) => node = target,
                None => break,
            },
            None => break,
        }
    }
    node.get("properties")
        .and_then(Value::as_object)
        .map(|props| props.keys().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the workspace")
            .to_path_buf()
    }

    // covers: QA-08
    // suite: unit
    // acceptance: 0.9
    // regression: 0.9
    #[test]
    fn schema_export_is_deterministic() {
        let first = generate(&root()).expect("generated");
        let second = generate(&root()).expect("generated again");
        assert_eq!(first, second);
        for (path, text) in &first {
            assert!(text.ends_with('\n'), "{path}");
            if path.ends_with(".json") {
                let value: Value = serde_json::from_str(text).expect("JSON");
                assert_eq!(&pretty(&value), text, "{path} has sorted keys");
                assert_eq!(value["$schema"], json!(DRAFT), "{path}");
                let id = value["$id"].as_str().expect("an $id");
                assert!(id.ends_with(path.trim_start_matches(&format!("{API_DIR}/"))));
            }
        }
    }

    // covers: QA-08
    // suite: unit
    // acceptance: 0.9
    // regression: 0.9
    #[test]
    fn every_method_event_setting_and_the_manifest_has_a_schema() {
        let files = generate(&root()).expect("generated");
        let has = |rel: &str| files.contains_key(&format!("{API_DIR}/{rel}"));
        for method in ALL {
            assert!(
                has(&format!("public/v1/{}.json", method.name)),
                "{}",
                method.name
            );
        }
        for notification in NOTIFICATIONS {
            assert!(has(&format!("public/v1/{}.json", notification.name)));
        }
        for kind in REGISTRY {
            assert!(
                has(&event_file(kind.event_type, kind.version)),
                "{}",
                kind.event_type
            );
        }
        for method in HOST_METHODS.iter().chain(MODULE_METHODS) {
            assert!(
                has(&format!("module/host-v1/{}.json", method.name)),
                "{}",
                method.name
            );
        }
        assert!(has("public/v1/errors.json"));
        assert!(has("settings/core.json"));
        assert!(has("module/manifest.v1.json"));
        assert!(has("INDEX.md"));

        // Params and result live in one file, with their shared types.
        let status = parsed(
            &files,
            &format!("{API_DIR}/public/v1/v1.events.subscribe.json"),
        )
        .expect("JSON");
        assert_eq!(status["x-notification"], json!("v1.events.event"));
        assert!(status["$defs"]["params"]["properties"]["types"].is_object());
        assert!(status["$defs"]["result"]["properties"]["subscription"].is_object());
        let core = parsed(&files, &format!("{API_DIR}/settings/core.json")).expect("JSON");
        assert_eq!(core["x-settings-version"], json!(CURRENT_VERSION));

        // The UI's helpers come from the same schemas.
        let js = &files[API_JS];
        assert!(js.starts_with(".pragma library\n"));
        assert!(js.contains("    systemStatus: \"v1.system.status\",\n"));
        assert!(js.contains("    eventsEvent: \"v1.events.event\",\n"));
        assert!(js.contains("    \"v1.events.subscribe\": Object.freeze([\"subscription\"]),\n"));
        assert!(js.contains("    \"not_found\",\n"));
    }

    // covers: QA-08
    // suite: unit
    // acceptance: 0.9
    // regression: 0.9
    #[test]
    fn bundles_share_types_and_old_event_versions_are_kept() {
        assert_eq!(js_name("v1.settings.get_schema"), "settingsGetSchema");
        assert_eq!(js_name("v2.system.status"), "v2SystemStatus");
        assert_eq!(
            parse_event_file("setting.changed.v12.json"),
            Some(("setting.changed", 12))
        );
        assert_eq!(parse_event_file("INDEX.md"), None);

        // Shared types are defined once; a type containing itself, directly
        // or through one of its defs, points at its new place.
        let shared = json!({"type": "string"});
        let params = json!({
            "properties": {"next": {"$ref": "#"}, "kid": {"$ref": "#/$defs/Kid"}, "s": {"$ref": "#/$defs/S"}},
            "$defs": {"Kid": {"properties": {"up": {"$ref": "#"}}}, "S": shared},
        });
        let result = json!({"properties": {"s": {"$ref": "#/$defs/S"}}, "$defs": {"S": shared}});
        let bundled =
            bundle_values(vec![("params", params.clone()), ("result", result)]).expect("bundled");
        let defs = &bundled["$defs"];
        assert_eq!(
            defs.as_object().expect("defs").keys().collect::<Vec<_>>(),
            ["Kid", "S", "params", "result"]
        );
        assert_eq!(
            defs["params"]["properties"]["next"]["$ref"],
            json!("#/$defs/params")
        );
        assert_eq!(
            defs["Kid"]["properties"]["up"]["$ref"],
            json!("#/$defs/params")
        );
        // Two different types of one name, or a type named like a part.
        let other = json!({"$defs": {"S": {"type": "integer"}}});
        let clash = bundle_values(vec![("params", params), ("result", other)]);
        assert!(clash.unwrap_err().to_string().contains("both called `S`"));
        let named = bundle_values(vec![
            ("params", json!({"$defs": {"result": {}}})),
            ("result", json!({})),
        ]);
        assert!(named.unwrap_err().to_string().contains("`result`"));

        // Names that would share a JS name are refused; strings are escaped.
        let mut out = String::new();
        let clash = names(
            &mut out,
            "methods",
            "x",
            &["v1.a.b_c".into(), "v1.a.b.c".into()],
        );
        assert!(
            clash
                .unwrap_err()
                .to_string()
                .contains("both be `methods.aBC`")
        );
        assert_eq!(js_string("a\"b"), "\"a\\\"b\"");

        // An old version of a registered type is carried over as committed;
        // anything else under events/ is not.
        let dir = tempfile::tempdir().expect("a folder");
        let events = dir.path().join(API_DIR).join("events");
        fs::create_dir_all(&events).expect("folders");
        fs::write(events.join("setting.changed.v0.json"), "{\"old\": true}\n").expect("written");
        fs::write(events.join("nope.thing.v0.json"), "{}\n").expect("written");
        // A version still registered keeps its generated schema.
        fs::write(
            events.join("setting.changed.v1.json"),
            "{\"stale\": true}\n",
        )
        .expect("written");
        let mut files: Files = [(
            "events/setting.changed.v1.json".to_owned(),
            "new\n".to_owned(),
        )]
        .into();
        keep_old_events(dir.path(), &mut files).expect("read");
        assert_eq!(
            files,
            [
                (
                    "events/setting.changed.v0.json".to_owned(),
                    "{\"old\": true}\n".to_owned()
                ),
                (
                    "events/setting.changed.v1.json".to_owned(),
                    "new\n".to_owned()
                ),
            ]
            .into()
        );
        fs::remove_file(events.join("setting.changed.v1.json")).expect("removed");

        // Committed files nothing generates are stale, and go on writing.
        let generated: Files = [(format!("{API_DIR}/a.json"), "{}\n".to_owned())].into();
        fs::write(events.join("stray.json"), "{}\n").expect("written");
        // Hidden files, such as Finder's, are nobody's business.
        fs::write(events.join(".DS_Store"), "junk").expect("written");
        let stale = schema_check::stale(&generated, &on_disk(dir.path()).expect("read"));
        assert!(
            stale.iter().any(|s| s.contains("a.json is missing")),
            "{stale:?}"
        );
        assert!(
            stale
                .iter()
                .any(|s| s.contains("stray.json is no longer generated"))
        );
        write_all(dir.path(), &generated).expect("written");
        assert!(!events.join("stray.json").exists());
        assert!(events.join(".DS_Store").exists());
        assert_eq!(
            fs::read_to_string(dir.path().join(API_DIR).join("a.json")).expect("read"),
            "{}\n"
        );
        assert!(schema_check::stale(&generated, &on_disk(dir.path()).expect("read")).is_empty());
    }
}
