//! `v1.ai.*`: Priya AI for programs. People ask with `v1.intent.submit`,
//! which streams; this waits for the whole answer.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Method;

/// `v1.ai.ask`
pub struct Ask;

impl Method for Ask {
    const NAME: &'static str = "v1.ai.ask";
    const SUMMARY: &'static str =
        "Asks a model and waits for the whole answer, for scripts and modules.";
    const READ_ONLY: bool = false;
    type Params = AskParams;
    type Result = AskResult;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskParams {
    pub prompt: String,
    /// What the answer is for, such as `fast` or `smart` (04-priya-ai §4).
    /// Recorded, but every role uses `ai.strategy` until 1.7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AskResult {
    pub answer: String,
    /// The model that answered.
    pub model: String,
}

/// `v1.ai.status`
pub struct Status;

impl Method for Status {
    const NAME: &'static str = "v1.ai.status";
    const SUMMARY: &'static str =
        "Shows which model answers, whether its key is stored and whether it can be reached.";
    const READ_ONLY: bool = true;
    type Params = StatusParams;
    type Result = AiStatus;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusParams {
    /// Also ask the provider whether it answers (no tokens; the keyring and
    /// the provider get 2 s each).
    #[serde(default)]
    pub check: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AiStatus {
    /// A model is chosen (`ai.strategy`) and can be used.
    pub configured: bool,
    /// The provider's name in settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The model runs on this computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<KeyState>,
    /// With `check`: whether the provider answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reachable: Option<bool>,
    /// What is wrong, in one plain sentence, if anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

/// Whether the provider's key is in the keyring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    Stored,
    Missing,
    /// A model on this computer needs none.
    NotNeeded,
}
