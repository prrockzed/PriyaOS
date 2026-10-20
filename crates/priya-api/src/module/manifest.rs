//! `module.toml`: what a module is, what it offers and what it may do
//! (02-core §5.1).
//!
//! [`ModuleManifest::parse`] reads one and checks it. Unknown keys are
//! refused, so a typo can't quietly drop a capability or a method. Every
//! problem found is reported at once, each as a plain sentence.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The module API version this build speaks.
pub const API_VERSION: &str = "v1";

/// A parsed and checked `module.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// The execution mode for the module's work unless the user says
    /// otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<DefaultMode>,
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
        let manifest: Self = toml_edit::de::from_str(text).map_err(|e| ManifestError {
            problems: vec![e.to_string().trim_end().to_owned()],
        })?;
        manifest.check()?;
        Ok(manifest)
    }

    pub fn name(&self) -> &str {
        &self.module.name
    }

    /// Whether the manifest grants `capability`.
    pub fn grants(&self, capability: &Capability) -> bool {
        self.requires.capabilities.contains(capability)
    }

    /// Checks what serde can't: names, versions, method prefixes and how
    /// the parts fit together.
    pub fn check(&self) -> Result<(), ManifestError> {
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
            (ModuleKind::Builtin, Some(_)) => {
                problems
                    .push("a builtin module runs inside priyad, so it has no `module.exec`".into());
            }
            _ => {}
        }
        let prefix = format!("{}.", module.name);
        let mut seen = Vec::new();
        for method in &self.provides.methods {
            let rest = method.strip_prefix(&prefix).unwrap_or_default();
            if !rest.split('.').all(is_word) {
                problems.push(format!(
                    "method `{method}` must be named `{prefix}<name>`, in lowercase letters, digits and `_`"
                ));
            }
            if seen.contains(&method) {
                problems.push(format!("method `{method}` is listed twice"));
            }
            seen.push(method);
        }
        for tool in &self.provides.tools {
            if !is_word(tool) {
                problems.push(format!(
                    "tool `{tool}` must be lowercase letters, digits, `_` or `-`, starting with a letter"
                ));
            }
        }
        if let Some(schema) = &self.provides.settings_schema
            && !is_relative_inside(schema)
        {
            problems.push(format!(
                "`provides.settings_schema` must be a file inside the module's folder; it is `{schema}`"
            ));
        }
        for surface in &self.provides.surfaces {
            if !is_word(&surface.id) {
                problems.push(format!(
                    "surface id `{}` must be lowercase letters, digits, `_` or `-`",
                    surface.id
                ));
            }
        }
        for pattern in &self.subscribes.events {
            if pattern.is_empty() || pattern.len() > 256 || pattern.contains(char::is_whitespace) {
                problems.push(format!("`{pattern}` isn't an event type or pattern"));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(ManifestError { problems })
        }
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
            !pre.is_empty()
                && pre
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
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

/// A relative path that stays inside its folder.
fn is_relative_inside(path: &str) -> bool {
    let path = std::path::Path::new(path);
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
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
}
