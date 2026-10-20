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
