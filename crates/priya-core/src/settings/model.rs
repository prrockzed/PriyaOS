//! The settings types and their defaults.
//!
//! Doc comments on fields become descriptions in the JSON Schema, which
//! editors and the Settings app show to people, so they are written for the
//! owner, not for developers.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::{Map, Value};

use super::names::ProviderName;
use super::strategy::Strategy;

/// The settings format this build writes. Each file is checked against it
/// and migrated up to it before the layers merge, so the merged value is
/// always this one.
pub const CURRENT_VERSION: u32 = 1;

/// Every setting, merged from all layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// The settings format this file is written in. PriyaOS updates it for
    /// you.
    pub settings_version: u32,
    pub ui: Ui,
    pub ai: Ai,
    pub log: Logging,
    pub modules: Modules,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            settings_version: CURRENT_VERSION,
            ui: Ui::default(),
            ai: Ai::default(),
            log: Logging::default(),
            modules: Modules::default(),
        }
    }
}

impl Settings {
    /// The JSON Schema of every core setting, for editors, the Settings app
    /// and the compatibility checks in 0.9.
    pub fn json_schema() -> schemars::Schema {
        schemars::schema_for!(Settings)
    }
}

/// How the desktop looks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Ui {
    /// Light or dark colours.
    pub theme: Theme,
    pub bar: Bar,
    /// Use fewer and shorter animations.
    pub reduced_motion: bool,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            // Dark by default: the Zenbook has an OLED screen.
            theme: Theme::Dark,
            bar: Bar::default(),
            reduced_motion: false,
        }
    }
}

/// Light or dark colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
}

/// The Priya bar, opened with Super+Space.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Bar {
    /// How wide the bar is, in pixels.
    #[serde(deserialize_with = "in_range::<_, 320, 7680>")]
    #[schemars(range(min = 320, max = 7680))]
    pub width_px: u32,
}

impl Default for Bar {
    fn default() -> Self {
        Self { width_px: 720 }
    }
}

/// Models and the providers that serve them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Ai {
    /// Which model answers. Until you set it, no model is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<Strategy>,
    /// The model providers you use, by a name you choose (lowercase letters,
    /// digits, - and _). The strategy names one: fixed:<name>/<model>.
    /// `local` (a model server on this computer) and `gemini` come built
    /// in; turn one off with `enabled = false`.
    pub providers: BTreeMap<ProviderName, Provider>,
    /// The longest answer a model may give, in tokens (about ¾ of a word
    /// each).
    #[serde(deserialize_with = "in_range::<_, 1, 1_000_000>")]
    #[schemars(range(min = 1, max = 1_000_000))]
    pub max_output_tokens: u32,
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            strategy: None,
            providers: default_providers(),
            max_output_tokens: 2048,
        }
    }
}

/// The providers PriyaOS knows out of the box (0.12): a local server such
/// as Ollama, and Gemini's free tier. Neither is used until
/// `ai.strategy` names one.
fn default_providers() -> BTreeMap<ProviderName, Provider> {
    let provider = |base_url: &str| Provider {
        kind: ProviderKind::OpenaiCompatible,
        base_url: Some(base_url.to_owned()),
        enabled: true,
    };
    [
        ("local", provider("http://127.0.0.1:11434/v1")),
        (
            "gemini",
            provider("https://generativelanguage.googleapis.com/v1beta/openai"),
        ),
    ]
    .into_iter()
    .map(|(name, provider)| {
        let name = ProviderName::try_from(name.to_owned());
        (name.expect("built-in provider names are valid"), provider)
    })
    .collect()
}

/// A model provider. Its API key is kept in the system keyring, never in a
/// settings file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    /// What kind of API it speaks.
    pub kind: ProviderKind,
    /// Its address, if not the usual one; for example a local server such as
    /// http://localhost:11434/v1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Turn it off without deleting it.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

/// What kind of API a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Anthropic's API (Claude).
    Anthropic,
    /// The OpenAI API, as spoken by Ollama, LM Studio and many others.
    OpenaiCompatible,
}

/// What PriyaOS writes to its log.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Logging {
    /// How much to log. The PRIYA_LOG environment variable, when set, wins.
    pub level: LogLevel,
}

/// How much to log, from least to most.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

/// Which modules run, and each module's own settings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Modules {
    /// The modules to start, by name.
    pub enabled: Vec<String>,
    /// Each module's own settings, as a table named after it
    /// (`[modules.weather]`). The module checks them against its own schema
    /// when it starts.
    #[serde(flatten)]
    pub sections: BTreeMap<String, Map<String, Value>>,
}

/// A whole number from `MIN` to `MAX`.
fn in_range<'de, D: Deserializer<'de>, const MIN: u32, const MAX: u32>(
    deserializer: D,
) -> Result<u32, D::Error> {
    let n = u32::deserialize(deserializer)?;
    if (MIN..=MAX).contains(&n) {
        Ok(n)
    } else {
        Err(D::Error::custom(format!(
            "must be a whole number from {MIN} to {MAX}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use toml_edit::{DocumentMut, Item, Table};

    use super::*;

    const SHIPPED: &str = include_str!("../../../../data/defaults/00-core.toml");

    fn parse(toml: &str) -> Result<Settings, String> {
        toml_edit::de::from_str(toml).map_err(|e| e.to_string())
    }

    /// Every leaf key under `table`, as dotted paths. Empty tables count as
    /// leaves, so an empty table such as `[modules.x]` must be there too.
    fn leaves(table: &Table, prefix: &str, out: &mut Vec<String>) {
        if table.is_empty() && !prefix.is_empty() {
            out.push(prefix.trim_end_matches('.').to_owned());
        }
        for (key, item) in table {
            match item {
                Item::Table(inner) => leaves(inner, &format!("{prefix}{key}."), out),
                _ => out.push(format!("{prefix}{key}")),
            }
        }
    }

    fn leaf_keys(toml: &str) -> Vec<String> {
        let document: DocumentMut = toml.parse().unwrap();
        let mut out = Vec::new();
        leaves(document.as_table(), "", &mut out);
        out.sort();
        out
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    // regression: 0.6
    #[test]
    fn shipped_defaults_match_the_code() {
        assert_eq!(parse(SHIPPED).unwrap(), Settings::default());

        // Key for key, so a new default can't be left out of the file.
        let from_code = toml_edit::ser::to_string_pretty(&Settings::default()).unwrap();
        assert_eq!(leaf_keys(SHIPPED), leaf_keys(&from_code));
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    // regression: 0.6
    #[test]
    fn core_sections_refuse_unknown_keys_and_module_sections_must_be_tables() {
        assert!(
            parse("[ui]\nshade = \"light\"\n")
                .unwrap_err()
                .contains("shade")
        );
        assert!(parse("colour = \"red\"\n").is_err());
        // A misspelt `enabled` would otherwise pass as a module called
        // "enable"; module sections must be tables.
        assert!(parse("[modules]\nenable = [\"weather\"]\n").is_err());
        assert!(parse("[modules]\nweather = 3\n").is_err());

        let settings =
            parse("[modules]\nenabled = [\"weather\"]\n[modules.weather]\ncity = \"Pune\"\n")
                .unwrap();
        assert_eq!(settings.modules.enabled, ["weather"]);
        assert_eq!(settings.modules.sections["weather"]["city"], "Pune");
        // Only what the file says changes; the rest keeps its default.
        assert_eq!(settings.ui, Ui::default());
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    // regression: 0.6
    #[test]
    fn providers_need_a_valid_name_and_kind_and_default_to_enabled() {
        let settings = parse(
            "[ai.providers.ollama]\nkind = \"openai_compatible\"\nbase_url = \"http://localhost:11434/v1\"\n",
        )
        .unwrap();
        let ollama = &settings.ai.providers["ollama"];
        assert!(ollama.enabled);
        assert_eq!(ollama.kind, ProviderKind::OpenaiCompatible);

        assert!(parse("[ai.providers.x]\nkind = \"openai\"\n").is_err());
        assert!(parse("[ai.providers.x]\nbase_url = \"http://x\"\n").is_err());
        let bad_name = parse("[ai.providers.Bad_Name]\nkind = \"anthropic\"\n").unwrap_err();
        assert!(bad_name.contains("can't be a provider name"), "{bad_name}");
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn a_local_server_and_gemini_ship_ready_but_unused() {
        let ai = Settings::default().ai;
        assert_eq!(ai.strategy, None, "no model until the owner picks one");
        let base = |name: &str| ai.providers[name].base_url.clone().unwrap();
        assert_eq!(base("local"), "http://127.0.0.1:11434/v1");
        assert_eq!(
            base("gemini"),
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
        assert!(
            ai.providers
                .values()
                .all(|p| p.enabled && p.kind == ProviderKind::OpenaiCompatible)
        );
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    // regression: 0.6
    #[test]
    fn numbers_outside_their_range_are_refused() {
        for (toml, ok) in [
            ("[ui.bar]\nwidth_px = 320\n", true),
            ("[ui.bar]\nwidth_px = 0\n", false),
            ("[ui.bar]\nwidth_px = 100000\n", false),
            ("[ai]\nmax_output_tokens = 1\n", true),
            ("[ai]\nmax_output_tokens = 0\n", false),
            ("[ai]\nmax_output_tokens = -5\n", false),
        ] {
            assert_eq!(parse(toml).is_ok(), ok, "{toml}");
        }
        let err = parse("[ui.bar]\nwidth_px = 0\n").unwrap_err();
        assert!(err.contains("from 320 to 7680"), "{err}");
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    // regression: 0.6
    #[test]
    fn json_schema_describes_every_core_section() {
        let schema = serde_json::to_value(Settings::json_schema()).unwrap();
        for section in ["settings_version", "ui", "ai", "log", "modules"] {
            assert!(
                schema["properties"].get(section).is_some(),
                "{section} missing"
            );
        }
        assert_eq!(schema["additionalProperties"], false);
        let text = schema.to_string();
        for expected in [
            r#""enum":["light","dark"]"#,
            "fixed:<provider>/<model>",
            r#""patternProperties":{"^[a-z0-9][a-z0-9_-]*$""#,
            r#""minimum":320"#,
            r#""maximum":7680"#,
        ] {
            assert!(text.contains(expected), "{expected} not in {text}");
        }
        // Descriptions are for people, so no rustdoc links.
        assert!(!text.contains("[`"), "{text}");
    }
}
