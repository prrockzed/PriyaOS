//! `v1.settings.*`: reading and changing settings (02-core §6).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Done, Method, NoParams};

/// `v1.settings.get`
pub struct Get;

impl Method for Get {
    const NAME: &'static str = "v1.settings.get";
    const SUMMARY: &'static str = "Shows settings, and where each one came from.";
    const READ_ONLY: bool = true;
    type Params = GetParams;
    type Result = GetResult;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetParams {
    /// A dotted key such as `ui.theme`; all settings when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Also say where each value came from.
    #[serde(default)]
    pub explain: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetResult {
    /// The value at the key: a single setting, or a table of them.
    pub value: Value,
    /// With `explain`: each value under the key and where it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origins: Option<Vec<ValueOrigin>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ValueOrigin {
    /// The setting's dotted key.
    pub key: String,
    /// Its value.
    pub value: Value,
    /// Which layer set it.
    pub layer: SettingLayer,
    /// The file that set it, if a file did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// The line in that file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// Where a setting's value comes from, lowest priority first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SettingLayer {
    /// Built into PriyaOS; no file sets it.
    BuiltIn,
    /// The shipped defaults, `/usr/share/priya/defaults`.
    Defaults,
    /// The administrator's, `/etc/priya`.
    System,
    /// Yours, `~/.config/priya`.
    User,
    /// The project's `.priya.toml`.
    Project,
    /// Given with one request.
    Task,
}

/// `v1.settings.set`
pub struct Set;

impl Method for Set {
    const NAME: &'static str = "v1.settings.set";
    const SUMMARY: &'static str = "Changes one setting in your settings file.";
    const READ_ONLY: bool = false;
    type Params = SetParams;
    type Result = Done;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetParams {
    /// The setting's dotted key, such as `ui.theme`.
    pub key: String,
    /// Its new value.
    pub value: Value,
}

/// `v1.settings.unset`
pub struct Unset;

impl Method for Unset {
    const NAME: &'static str = "v1.settings.unset";
    const SUMMARY: &'static str =
        "Removes one setting from your settings file, so its default applies.";
    const READ_ONLY: bool = false;
    type Params = UnsetParams;
    type Result = Done;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnsetParams {
    /// The setting's dotted key, such as `ui.theme`.
    pub key: String,
}

/// `v1.settings.schema`
pub struct GetSchema;

impl Method for GetSchema {
    const NAME: &'static str = "v1.settings.schema";
    const SUMMARY: &'static str = "Gives the JSON Schema of every setting.";
    const READ_ONLY: bool = true;
    type Params = NoParams;
    type Result = SchemaResult;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SchemaResult {
    /// The JSON Schema of the settings.
    pub schema: Value,
}
