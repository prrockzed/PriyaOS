//! `module.toml`: what a module is, what it offers and what it may do
//! (02-core §5.1).
//!
//! [`ModuleManifest::parse`] reads one and checks it. Unknown keys are
//! refused, so a typo can't quietly drop a capability or a method. A
//! manifest that reads as TOML has every other problem reported at once,
//! each as one plain sentence. Deserializing a `ModuleManifest` from any
//! format runs the same checks, so an unchecked one can't exist.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The module API version this build speaks.
pub const API_VERSION: &str = "v1";

/// A parsed and checked `module.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, try_from = "Unchecked")]
pub struct ModuleManifest {
    pub module: ModuleInfo,
    #[serde(default)]
    pub provides: Provides,
    #[serde(default)]
    pub subscribes: Subscribes,
    #[serde(default)]
    pub requires: Requires,
}

/// `[module]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModuleInfo {
    /// 2-32 lowercase letters, digits or `-`, starting with a letter.
    #[schemars(extend("pattern" = "^[a-z][a-z0-9-]{1,31}$"))]
    pub name: String,
    /// `MAJOR.MINOR.PATCH`, optionally with a `-pre-release` part.
    pub version: String,
    /// The module API version: `v1`.
    pub api: String,
    pub kind: ModuleKind,
    /// The program to run, for a process module: absolute, or relative to
    /// the manifest's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// How a module runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModuleKind {
    /// Rust code inside priyad, over in-process channels.
    Builtin,
    /// Its own process, any language, JSON-RPC on stdio.
    Process,
}

impl fmt::Display for ModuleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Builtin => "builtin",
            Self::Process => "process",
        })
    }
}

/// `[provides]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provides {
    /// Methods callable through `v1.module.call`, each named
    /// `<module>.<method>`.
    #[serde(default)]
    pub methods: Vec<String>,
    /// Example requests Priya AI uses to route intents here.
    #[serde(default)]
    pub intents: Vec<String>,
    /// Tools offered to agents over MCP.
    #[serde(default)]
    pub tools: Vec<String>,
    /// The JSON Schema of the module's `[modules.<name>]` settings,
    /// relative to the manifest's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_schema: Option<String>,
    #[serde(default)]
    pub surfaces: Vec<Surface>,
}

/// A place in the desktop the module shows up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    pub kind: SurfaceKind,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    BarItem,
    Command,
}

/// `[subscribes]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Subscribes {
    /// Event types, or patterns such as `module.*`.
    #[serde(default)]
    pub events: Vec<String>,
}

/// `[requires]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// The execution mode for the module's work unless the user says
    /// otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<DefaultMode>,
}

/// A manifest as read, before its capabilities are parsed and the whole
/// is checked. Its schema is the manifest's.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ModuleManifest")]
struct Unchecked {
    module: ModuleInfo,
    #[serde(default)]
    provides: Provides,
    #[serde(default)]
    subscribes: Subscribes,
    #[serde(default)]
    requires: UncheckedRequires,
}

#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "Requires")]
struct UncheckedRequires {
    #[serde(default)]
    #[schemars(with = "Vec<Capability>")]
    capabilities: Vec<String>,
    /// The execution mode for the module's work unless the user says
    /// otherwise.
    #[serde(default)]
    default_mode: Option<DefaultMode>,
}

impl TryFrom<Unchecked> for ModuleManifest {
    type Error = ManifestError;

    fn try_from(raw: Unchecked) -> Result<Self, Self::Error> {
        let mut problems = Vec::new();
        let mut capabilities = Vec::new();
        for text in raw.requires.capabilities {
            match text.parse::<Capability>() {
                Ok(capability) if capabilities.contains(&capability) => {
                    problems.push(format!("capability `{capability}` is listed twice"));
                }
                Ok(capability) => capabilities.push(capability),
                Err(problem) => problems.push(problem),
            }
        }
        let manifest = Self {
            module: raw.module,
            provides: raw.provides,
            subscribes: raw.subscribes,
            requires: Requires {
                capabilities,
                default_mode: raw.requires.default_mode,
            },
        };
        problems.extend(manifest.problems());
        if problems.is_empty() {
            Ok(manifest)
        } else {
            Err(ManifestError { problems })
        }
    }
}

/// An execution mode a module may default to. `direct_raw` is never a
/// default (03-execution-and-sandbox §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DefaultMode {
    ReadOnly,
    Sandbox,
    Direct,
}

/// Something a module may do. Every host call is checked against these.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    /// `events.emit`: record its own events.
    EventsEmit,
    /// `settings.read:<prefix>`: read settings under a key prefix (its own
    /// `modules.<name>` section needs nothing).
    SettingsRead(String),
    /// `ai.role:<role>`: ask Priya AI with a role (0.12).
    AiRole(String),
    /// `network:<label>`: reach the network for a labelled purpose.
    Network(String),
    /// `api.read`: call read-only public API methods.
    ApiRead,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventsEmit => f.write_str("events.emit"),
            Self::SettingsRead(prefix) => write!(f, "settings.read:{prefix}"),
            Self::AiRole(role) => write!(f, "ai.role:{role}"),
            Self::Network(label) => write!(f, "network:{label}"),
            Self::ApiRead => f.write_str("api.read"),
        }
    }
}

/// The capabilities a manifest may ask for, for messages.
const KNOWN: &str =
    "events.emit, settings.read:<prefix>, ai.role:<role>, network:<label> and api.read";

impl FromStr for Capability {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let named = |value: &str, what: &str, valid: fn(&str) -> bool, make: fn(String) -> Self| {
            if valid(value) {
                Ok(make(value.to_owned()))
            } else {
                Err(format!(
                    "`{s}` needs a {what} after the colon, like `{}`",
                    example(s)
                ))
            }
        };
        match s {
            "events.emit" => Ok(Self::EventsEmit),
            "api.read" => Ok(Self::ApiRead),
            _ if s.starts_with("helper.") => Err(format!(
                "`{s}`: root helper capabilities aren't available to modules until Phase 2"
            )),
            _ => match s.split_once(':') {
                Some(("settings.read", prefix)) => {
                    named(prefix, "settings key", is_key_prefix, Self::SettingsRead)
                }
                Some(("ai.role", role)) => named(role, "role name", is_word, Self::AiRole),
                Some(("network", label)) => named(label, "label", is_word, Self::Network),
                _ => Err(format!(
                    "`{s}` isn't a capability; modules may ask for {KNOWN}"
                )),
            },
        }
    }
}

fn example(capability: &str) -> &'static str {
    match capability.split_once(':').map(|(head, _)| head) {
        Some("settings.read") => "settings.read:ui",
        Some("ai.role") => "ai.role:fast",
        _ => "network:mirrors",
    }
}

impl Serialize for Capability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Capability {
    fn schema_name() -> Cow<'static, str> {
        "Capability".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^(events\\.emit|api\\.read|settings\\.read:[a-z0-9_]+(\\.[a-z0-9_]+)*|(ai\\.role|network):[a-z][a-z0-9_-]{0,63})$"
        })
    }
}

/// Why a manifest was refused: every problem, one sentence each.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", .problems.join("\n"))]
pub struct ManifestError {
    pub problems: Vec<String>,
}

impl ModuleManifest {
    /// Reads `module.toml` text and checks it.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let raw: Unchecked = toml_edit::de::from_str(text).map_err(|e| ManifestError {
            problems: vec![toml_problem(text, &e)],
        })?;
        Self::try_from(raw)
    }

    pub fn name(&self) -> &str {
        &self.module.name
    }

    /// Whether the manifest asks for exactly `capability`. Settings are
    /// checked with [`may_read_setting`](Self::may_read_setting) instead.
    pub fn grants(&self, capability: &Capability) -> bool {
        self.requires.capabilities.contains(capability)
    }

    /// Whether the module may read the setting `key` (given as its parts):
    /// anything in its own `modules.<name>` section, and anything under a
    /// `settings.read:<prefix>` it asks for. Prefixes match whole parts,
    /// so `ui` covers `ui.theme` but not `uix`.
    pub fn may_read_setting<S: AsRef<str>>(&self, key: &[S]) -> bool {
        let starts_with = |prefix: &[&str]| {
            key.len() >= prefix.len() && prefix.iter().zip(key).all(|(p, k)| *p == k.as_ref())
        };
        starts_with(&["modules", &self.module.name])
            || self
                .requires
                .capabilities
                .iter()
                .any(|capability| match capability {
                    Capability::SettingsRead(prefix) => {
                        starts_with(&prefix.split('.').collect::<Vec<_>>())
                    }
                    _ => false,
                })
    }

    /// Checks what serde can't: names, versions, method prefixes and how
    /// the parts fit together. (Capabilities were checked as they were
    /// read.)
    pub fn check(&self) -> Result<(), ManifestError> {
        let problems = self.problems();
        if problems.is_empty() {
            Ok(())
        } else {
            Err(ManifestError { problems })
        }
    }

    fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let module = &self.module;
        if !is_module_name(&module.name) {
            problems.push(format!(
                "`module.name` must be 2-32 lowercase letters, digits or `-`, starting with a letter; it is `{}`",
                module.name
            ));
        } else if is_reserved(&module.name) {
            problems.push(format!(
                "`module.name` can't be `{}`: PriyaOS uses that name itself",
                module.name
            ));
        }
        if !is_version(&module.version) {
            problems.push(format!(
                "`module.version` must look like 1.2.3; it is `{}`",
                module.version
            ));
        }
        if module.api != API_VERSION {
            problems.push(format!(
                "`module.api` must be \"{API_VERSION}\"; this PriyaOS doesn't speak `{}`",
                module.api
            ));
        }
        match (module.kind, &module.exec) {
            (ModuleKind::Process, None) => {
                problems.push("a process module needs `module.exec`, the program to run".into());
            }
            (ModuleKind::Process, Some(exec)) if exec.trim().is_empty() => {
                problems.push("`module.exec` is empty".into());
            }
            (ModuleKind::Process, Some(exec)) if exec.trim() != exec => {
                problems.push(format!("`module.exec` has spaces around it: `{exec}`"));
            }
            (ModuleKind::Process, Some(exec))
                if !exec.starts_with('/') && !is_relative_inside(exec) =>
            {
                problems.push(format!(
                    "`module.exec` must be an absolute path or a file inside the module's folder; it is `{exec}`"
                ));
            }
            (ModuleKind::Builtin, Some(_)) => {
                problems
                    .push("a builtin module runs inside priyad, so it has no `module.exec`".into());
            }
            _ => {}
        }
        let prefix = format!("{}.", module.name);
        for (i, method) in self.provides.methods.iter().enumerate() {
            let rest = method.strip_prefix(&prefix).unwrap_or_default();
            // With a bad name every method would be reported too.
            if is_module_name(&module.name) && !rest.split('.').all(is_method_word) {
                problems.push(format!(
                    "method `{method}` must be named `{prefix}<name>`, in lowercase letters, digits and `_`"
                ));
            }
            if self.provides.methods[..i].contains(method) {
                problems.push(format!("method `{method}` is listed twice"));
            }
        }
        for (i, tool) in self.provides.tools.iter().enumerate() {
            if !is_word(tool) {
                problems.push(format!(
                    "tool `{tool}` must be lowercase letters, digits, `_` or `-`, starting with a letter"
                ));
            }
            if self.provides.tools[..i].contains(tool) {
                problems.push(format!("tool `{tool}` is listed twice"));
            }
        }
        if let Some(schema) = &self.provides.settings_schema
            && !is_relative_inside(schema)
        {
            problems.push(format!(
                "`provides.settings_schema` must be a file inside the module's folder; it is `{schema}`"
            ));
        }
        for (i, surface) in self.provides.surfaces.iter().enumerate() {
            if !is_word(&surface.id) {
                problems.push(format!(
                    "surface id `{}` must be lowercase letters, digits, `_` or `-`",
                    surface.id
                ));
            }
            if self.provides.surfaces[..i]
                .iter()
                .any(|s| s.id == surface.id)
            {
                problems.push(format!("surface id `{}` is used twice", surface.id));
            }
        }
        for pattern in &self.subscribes.events {
            if pattern.is_empty() || pattern.len() > 256 || pattern.contains(char::is_whitespace) {
                problems.push(format!("`{pattern}` isn't an event type or pattern"));
            }
        }
        problems
    }
}

/// A TOML or structure error as one sentence with its line.
fn toml_problem(text: &str, error: &toml_edit::de::Error) -> String {
    let message = error.message().trim();
    match error.span() {
        Some(span) => {
            let line = text[..span.start.min(text.len())].matches('\n').count() + 1;
            format!("line {line}: {message}")
        }
        None => message.to_owned(),
    }
}

/// `[a-z][a-z0-9-]{1,31}`
fn is_module_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (2..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Names that would clash with the protocol or the public API.
fn is_reserved(name: &str) -> bool {
    matches!(name, "host" | "priya" | "system")
        || name
            .strip_prefix('v')
            .is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))
}

/// `MAJOR.MINOR.PATCH[-pre]`, numbers without leading zeros.
fn is_version(version: &str) -> bool {
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    };
    let numbers: Vec<&str> = core.split('.').collect();
    numbers.len() == 3
        && numbers.iter().all(|n| {
            !n.is_empty()
                && n.len() <= 9
                && n.bytes().all(|b| b.is_ascii_digit())
                && (n.len() == 1 || !n.starts_with('0'))
        })
        && pre.is_none_or(|pre| {
            pre.split('.').all(|part| {
                !part.is_empty()
                    && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    && (!part.bytes().all(|b| b.is_ascii_digit())
                        || part == "0"
                        || !part.starts_with('0'))
            })
        })
}

/// A lowercase identifier: `[a-z][a-z0-9_-]{0,63}`.
fn is_word(word: &str) -> bool {
    let bytes = word.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-')
}

/// One part of a method name: `[a-z][a-z0-9_]{0,63}`.
fn is_method_word(word: &str) -> bool {
    is_word(word) && !word.contains('-')
}

/// A dotted settings key without quoting, such as `ai.providers`.
fn is_key_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix.len() <= 256
        && prefix.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        })
}

/// A relative path to a file that stays inside its folder (`./` allowed).
fn is_relative_inside(path: &str) -> bool {
    use std::path::Component;
    let components: Vec<Component> = std::path::Path::new(path).components().collect();
    components.iter().any(|c| matches!(c, Component::Normal(_)))
        && components
            .iter()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ECHO: &str = r#"
[module]
name = "echo"
version = "0.1.0"
api = "v1"
kind = "process"
exec = "echo-module"
description = "Says things back"

[provides]
methods = ["echo.say", "echo.emit"]
intents = ["say something back"]
tools = ["echo_say"]
settings_schema = "settings.schema.json"
surfaces = [{ kind = "command", id = "say", title = "Say something" }]

[subscribes]
events = ["setting.changed", "module.*"]

[requires]
capabilities = ["events.emit", "settings.read:ui", "ai.role:fast", "network:weather", "api.read"]
default_mode = "read_only"
"#;

    fn problems(text: &str) -> Vec<String> {
        ModuleManifest::parse(text).unwrap_err().problems
    }

    /// The echo manifest with `from` replaced by `to`.
    fn echo_with(from: &str, to: &str) -> String {
        assert!(ECHO.contains(from), "{from}");
        ECHO.replacen(from, to, 1)
    }

    // covers: CORE-02
    // suite: unit
    // acceptance: 0.8
    #[test]
    fn manifest_rejects_bad_name_unknown_capability_and_helper_caps() {
        let manifest = ModuleManifest::parse(ECHO).unwrap();
        assert_eq!(manifest.name(), "echo");
        assert_eq!(manifest.module.kind, ModuleKind::Process);
        assert!(manifest.grants(&Capability::SettingsRead("ui".into())));
        assert!(!manifest.grants(&Capability::SettingsRead("ai".into())));
        assert_eq!(manifest.requires.capabilities.len(), 5);
        // Capabilities print back as written.
        let written: Vec<String> = manifest
            .requires
            .capabilities
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            written,
            [
                "events.emit",
                "settings.read:ui",
                "ai.role:fast",
                "network:weather",
                "api.read"
            ]
        );

        for name in ["Echo", "e", "1echo", "echo_x", &"e".repeat(33), "ec ho"] {
            let found = problems(&echo_with("name = \"echo\"", &format!("name = \"{name}\"")));
            assert!(
                found.iter().any(|p| p.contains("`module.name` must be")),
                "{name}: {found:?}"
            );
        }
        assert!(
            problems(&echo_with("name = \"echo\"", "name = \"host\""))[0].contains("PriyaOS uses")
        );
        assert!(
            problems(&echo_with("name = \"echo\"", "name = \"v2\""))[0].contains("PriyaOS uses")
        );

        let unknown = problems(&echo_with("\"api.read\"", "\"files.write\""));
        assert_eq!(unknown.len(), 1);
        assert!(
            unknown[0].contains("`files.write` isn't a capability"),
            "{unknown:?}"
        );
        let helper = problems(&echo_with("\"api.read\"", "\"helper.sandbox\""));
        assert!(helper[0].contains("until Phase 2"), "{helper:?}");
        for bare in [
            "settings.read:",
            "ai.role:",
            "network:",
            "settings.read:UI",
            "network:a b",
        ] {
            let found = problems(&echo_with("\"api.read\"", &format!("\"{bare}\"")));
            assert!(found[0].contains("after the colon"), "{bare}: {found:?}");
        }

        // Unknown keys are refused, wherever they are.
        let typo = problems(&echo_with("[requires]", "[requires]\ncapability = []"));
        assert!(typo[0].contains("capability"), "{typo:?}");
        assert!(!problems(&format!("{ECHO}\n[extra]\nx = 1\n")).is_empty());
        assert!(
            !problems(&echo_with(
                "kind = \"process\"",
                "kind = \"process\"\nexecutable = \"x\""
            ))
            .is_empty()
        );
    }

    // covers: CORE-02
    // suite: unit
    #[test]
    fn manifest_checks_versions_methods_and_kinds() {
        let api = problems(&echo_with("api = \"v1\"", "api = \"v2\""));
        assert_eq!(
            api,
            ["`module.api` must be \"v1\"; this PriyaOS doesn't speak `v2`"]
        );
        for version in [
            "1",
            "1.2",
            "01.2.3",
            "1.2.3.4",
            "1.2.x",
            "1.2.3-",
            "1.2.3-a b",
        ] {
            let found = problems(&echo_with(
                "version = \"0.1.0\"",
                &format!("version = \"{version}\""),
            ));
            assert!(found[0].contains("module.version"), "{version}");
        }
        assert!(ModuleManifest::parse(&echo_with("0.1.0", "10.0.1-rc.1")).is_ok());

        let unprefixed = problems(&echo_with("\"echo.emit\"", "\"other.emit\""));
        assert!(
            unprefixed[0].contains("must be named `echo.<name>`"),
            "{unprefixed:?}"
        );
        for bad in ["echo.", "echo.Say", "echo..say", "echo"] {
            assert!(
                !problems(&echo_with("\"echo.emit\"", &format!("\"{bad}\""))).is_empty(),
                "{bad}"
            );
        }
        assert!(ModuleManifest::parse(&echo_with("\"echo.emit\"", "\"echo.deep.emit_2\"")).is_ok());
        let twice = problems(&echo_with("\"echo.emit\"", "\"echo.say\""));
        assert_eq!(twice, ["method `echo.say` is listed twice"]);

        let no_exec = problems(&echo_with("exec = \"echo-module\"\n", ""));
        assert_eq!(
            no_exec,
            ["a process module needs `module.exec`, the program to run"]
        );
        assert_eq!(
            problems(&echo_with("\"echo-module\"", "\" \"")),
            ["`module.exec` is empty"]
        );
        let builtin = echo_with("kind = \"process\"", "kind = \"builtin\"");
        assert!(problems(&builtin)[0].contains("no `module.exec`"));
        let builtin =
            ModuleManifest::parse(&builtin.replace("exec = \"echo-module\"\n", "")).unwrap();
        assert_eq!(builtin.module.kind.to_string(), "builtin");

        for path in ["/etc/schema.json", "../schema.json", "a/../../b", ""] {
            let found = problems(&echo_with(
                "\"settings.schema.json\"",
                &format!("\"{path}\""),
            ));
            assert!(found[0].contains("inside the module's folder"), "{path}");
        }
        assert!(!problems(&echo_with("\"echo_say\"", "\"Echo Say\"")).is_empty());
        assert!(!problems(&echo_with("id = \"say\"", "id = \"Say!\"")).is_empty());
        assert!(!problems(&echo_with("\"module.*\"", "\"module .*\"")).is_empty());
        assert!(!problems(&echo_with("\"read_only\"", "\"direct_raw\"")).is_empty());

        // Several problems are all reported.
        let many = problems(&echo_with("api = \"v1\"", "api = \"v9\"").replace("0.1.0", "x"));
        assert_eq!(many.len(), 2, "{many:?}");
        assert_eq!(
            ModuleManifest::parse("[module]\nname = \"x\"\n")
                .unwrap_err()
                .problems
                .len(),
            1
        );

        // The smallest manifest: no optional tables.
        let minimal = ModuleManifest::parse(
            "[module]\nname = \"mini\"\nversion = \"1.0.0\"\napi = \"v1\"\nkind = \"builtin\"\n",
        )
        .unwrap();
        assert!(minimal.provides.methods.is_empty() && minimal.requires.capabilities.is_empty());
    }

    // covers: CORE-02
    // suite: unit
    #[test]
    fn manifest_round_trips_and_has_a_schema() {
        let manifest = ModuleManifest::parse(ECHO).unwrap();
        let json = serde_json::to_value(&manifest).unwrap();
        assert_eq!(json["requires"]["capabilities"][1], "settings.read:ui");
        assert_eq!(
            serde_json::from_value::<ModuleManifest>(json).unwrap(),
            manifest
        );
        let schema = serde_json::to_value(schemars::schema_for!(ModuleManifest)).unwrap();
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema["$defs"]["Capability"]["pattern"].is_string());
    }

    // covers: CORE-02
    // suite: unit
    #[test]
    fn manifest_reports_every_problem_and_checks_every_path() {
        // A bad name and two bad capabilities: all three, one line each.
        let text = echo_with("name = \"echo\"", "name = \"Echo\"").replace(
            "\"api.read\"",
            "\"files.write\", \"helper.x\", \"events.emit\"",
        );
        let found = problems(&text);
        assert_eq!(found.len(), 4, "{found:?}");
        assert!(found.iter().all(|p| !p.contains('\n')));
        assert!(
            found
                .iter()
                .any(|p| p == "capability `events.emit` is listed twice")
        );
        // A TOML mistake is one sentence with its line.
        let typo = problems(&echo_with("[requires]", "[requires]\ncapability = []"));
        assert!(
            typo[0].starts_with("line 21: unknown field `capability`"),
            "{typo:?}"
        );
        assert!(!typo[0].contains('\n'));

        // Deserializing from anything runs the same checks.
        let mut json = serde_json::to_value(ModuleManifest::parse(ECHO).unwrap()).unwrap();
        json["module"]["api"] = "v9".into();
        assert!(serde_json::from_value::<ModuleManifest>(json).is_err());

        for name in ["priya", "system"] {
            let text = echo_with("name = \"echo\"", &format!("name = \"{name}\""));
            assert!(problems(&text)[0].contains("PriyaOS uses"), "{name}");
        }
        assert!(
            problems(&echo_with("\"echo.emit\"", "\"echo.say-it\""))[0].contains("must be named")
        );
        assert_eq!(
            problems(&echo_with(
                "tools = [\"echo_say\"]",
                "tools = [\"a\", \"a\"]"
            )),
            ["tool `a` is listed twice"]
        );
        let surfaces = "surfaces = [{ kind = \"command\", id = \"say\" }, { kind = \"bar_item\", id = \"say\" }]";
        let twice = problems(&echo_with(
            "surfaces = [{ kind = \"command\", id = \"say\", title = \"Say something\" }]",
            surfaces,
        ));
        assert_eq!(twice, ["surface id `say` is used twice"]);

        // Paths: `./` is fine; nothing may climb out; exec may be absolute.
        assert!(
            ModuleManifest::parse(&echo_with(
                "\"settings.schema.json\"",
                "\"./settings.schema.json\""
            ))
            .is_ok()
        );
        assert!(ModuleManifest::parse(&echo_with("\"settings.schema.json\"", "\".\"")).is_err());
        assert!(
            ModuleManifest::parse(&echo_with(
                "\"echo-module\"",
                "\"/usr/lib/priya/modules/echo\""
            ))
            .is_ok()
        );
        assert!(ModuleManifest::parse(&echo_with("\"echo-module\"", "\"bin/echo\"")).is_ok());
        for exec in ["../../bin/sh", " echo-module", "echo-module "] {
            let found = problems(&echo_with("\"echo-module\"", &format!("\"{exec}\"")));
            assert!(found[0].contains("module.exec"), "{exec}: {found:?}");
        }

        // Pre-release parts are dot-separated, numbers without leading zeros.
        for version in ["0.1.0-01", "0.1.0-a..b", "0.1.0-.a"] {
            assert!(
                !problems(&echo_with("0.1.0", version)).is_empty(),
                "{version}"
            );
        }
        for version in ["0.1.0-0", "0.1.0-alpha.10", "0.1.0-x-y.0a"] {
            assert!(
                ModuleManifest::parse(&echo_with("0.1.0", version)).is_ok(),
                "{version}"
            );
        }
    }

    // covers: CORE-02
    // suite: unit
    #[test]
    fn settings_reads_follow_the_own_section_and_whole_key_parts() {
        let manifest = ModuleManifest::parse(ECHO).unwrap();
        let may = |key: &str| manifest.may_read_setting(&key.split('.').collect::<Vec<_>>());
        assert!(may("modules.echo.prefix"), "its own section");
        assert!(may("modules.echo"));
        assert!(!may("modules.echoes.x"), "another module's");
        assert!(!may("modules"));
        assert!(may("ui.theme") && may("ui"), "settings.read:ui");
        assert!(!may("uix.theme"), "whole parts only");
        assert!(!may("ai.strategy"));
        let deep =
            ModuleManifest::parse(&echo_with("settings.read:ui", "settings.read:ai.providers"))
                .unwrap();
        let may = |key: &str| deep.may_read_setting(&key.split('.').collect::<Vec<_>>());
        assert!(may("ai.providers.ollama.kind") && !may("ai") && !may("ai.strategy"));
    }
}
