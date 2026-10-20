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
    pub value: Value,
    /// With `explain`: each value under the key and where it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origins: Option<Vec<ValueOrigin>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ValueOrigin {
    pub key: String,
    pub value: Value,
    /// `defaults`, `system`, `user`, `project` or `task`, or `built_in` for
    /// a default no file sets.
    pub layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// `v1.settings.set`
pub struct Set;

impl Method for Set {
    const NAME: &'static str = "v1.settings.set";
    const SUMMARY: &'static str = "Changes one setting in your settings file.";
    type Params = SetParams;
    type Result = Done;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetParams {
    pub key: String,
    pub value: Value,
}

/// `v1.settings.unset`
pub struct Unset;

impl Method for Unset {
    const NAME: &'static str = "v1.settings.unset";
    const SUMMARY: &'static str =
        "Removes one setting from your settings file, so its default applies.";
    type Params = UnsetParams;
    type Result = Done;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnsetParams {
    pub key: String,
}

/// `v1.settings.schema`
pub struct GetSchema;

impl Method for GetSchema {
    const NAME: &'static str = "v1.settings.schema";
    const SUMMARY: &'static str = "Gives the JSON Schema of every setting.";
    type Params = NoParams;
    type Result = SchemaResult;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SchemaResult {
    pub schema: Value,
}
