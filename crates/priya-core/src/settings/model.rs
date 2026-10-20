//! The settings types and their defaults.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::strategy::Strategy;

/// The settings format this build writes. Older files are migrated up to it.
pub const CURRENT_VERSION: u32 = 1;

/// Every setting, merged from all layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// The format of the file that set it; see [`CURRENT_VERSION`].
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
    pub theme: Theme,
    pub bar: Bar,
    /// Fewer and shorter animations.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
}

/// The Priya bar (Super+Space).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Bar {
    /// Width in logical pixels.
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
    /// Which model answers. Unset means no model is configured yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<Strategy>,
    /// Providers by name, as used in the strategy (`fixed:<name>/<model>`).
    pub providers: BTreeMap<String, Provider>,
    /// The longest answer a model may give, in tokens.
    pub max_output_tokens: u32,
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            strategy: None,
            providers: BTreeMap::new(),
            max_output_tokens: 2048,
        }
    }
}

/// A model provider. Its API key is kept in the system keyring, never here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub kind: ProviderKind,
    /// Where its API is, when not the provider's usual address (a local
    /// server, say).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    /// Anything that speaks the OpenAI API, such as Ollama or LM Studio.
    OpenaiCompatible,
}

/// What priyad logs. `PRIYA_LOG`, when set, wins.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Logging {
    pub level: LogLevel,
}

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
    /// Module names to start.
    pub enabled: Vec<String>,
    /// `modules.<name>.*`, checked against that module's schema (0.8).
    #[serde(flatten)]
    pub sections: BTreeMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use toml_edit::{DocumentMut, Item, Table};

    use super::*;

    const SHIPPED: &str = include_str!("../../../../data/defaults/00-core.toml");

    /// Every leaf key under `table`, as dotted paths. Empty tables count as
    /// leaves, so `[ai.providers]` must be there too.
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
    #[test]
    fn shipped_defaults_match_the_code() {
        let shipped: Settings = toml_edit::de::from_str(SHIPPED).unwrap();
        assert_eq!(shipped, Settings::default());

        // Key for key, so a new default can't be left out of the file.
        let from_code = toml_edit::ser::to_string_pretty(&Settings::default()).unwrap();
        assert_eq!(leaf_keys(SHIPPED), leaf_keys(&from_code));
        assert_eq!(Settings::default().ui.theme, Theme::Dark);
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn core_sections_refuse_unknown_keys_but_module_sections_are_open() {
        let typo = toml_edit::de::from_str::<Settings>("[ui]\nshade = \"light\"\n");
        assert!(typo.unwrap_err().to_string().contains("shade"));
        let top = toml_edit::de::from_str::<Settings>("colour = \"red\"\n");
        assert!(top.is_err());

        let settings: Settings = toml_edit::de::from_str(
            "[modules]\nenabled = [\"weather\"]\n[modules.weather]\ncity = \"Pune\"\n",
        )
        .unwrap();
        assert_eq!(settings.modules.enabled, ["weather"]);
        assert_eq!(settings.modules.sections["weather"]["city"], "Pune");
        // Only what the file says changes; the rest keeps its default.
        assert_eq!(settings.ui, Ui::default());
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn providers_default_to_enabled_and_need_a_known_kind() {
        let settings: Settings = toml_edit::de::from_str(
            "[ai.providers.ollama]\nkind = \"openai_compatible\"\nbase_url = \"http://localhost:11434/v1\"\n",
        )
        .unwrap();
        let ollama = &settings.ai.providers["ollama"];
        assert!(ollama.enabled);
        assert_eq!(ollama.kind, ProviderKind::OpenaiCompatible);
        assert!(
            toml_edit::de::from_str::<Settings>("[ai.providers.x]\nkind = \"openai\"\n").is_err()
        );
    }

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn json_schema_describes_every_core_section() {
        let schema = serde_json::to_value(Settings::json_schema()).unwrap();
        for section in ["settings_version", "ui", "ai", "log", "modules"] {
            assert!(
                schema["properties"].get(section).is_some(),
                "{section} missing"
            );
        }
        let text = schema.to_string();
        assert!(text.contains(r#""enum":["light","dark"]"#), "{text}");
        assert!(text.contains("fixed:<provider>/<model>"));
        assert_eq!(schema["additionalProperties"], false);
    }
}
