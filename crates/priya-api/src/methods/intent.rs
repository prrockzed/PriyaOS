//! `v1.intent.*`: asking Priya (02-core §4). Phase 0 only answers; tasks
//! that change things come with Phase 3.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Done, Method};
use crate::error::ApiError;

/// The notification that carries an answer as it streams.
pub const STREAM_NOTIFICATION: &str = "v1.intent.stream";

/// The longest question accepted, in characters: far more than anyone
/// types, and it keeps one request from filling a model's whole window.
pub const MAX_TEXT_CHARS: usize = 20_000;

/// `v1.intent.submit`
pub struct Submit;

impl Method for Submit {
    const NAME: &'static str = "v1.intent.submit";
    const SUMMARY: &'static str = "Asks Priya something; the answer streams back as it is written.";
    const READ_ONLY: bool = false;
    const NOTIFICATION: Option<&'static str> = Some(STREAM_NOTIFICATION);
    type Params = SubmitParams;
    type Result = Submitted;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmitParams {
    /// The question, in any language.
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<IntentContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<IntentOverrides>,
}

/// Where the question comes from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentContext {
    /// Continue this conversation; a new one starts when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    /// The folder the asker is in (used from Phase 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Who asks: `cli`, `bar`, … (`api` when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
}

/// Choices that win over settings for this question only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentOverrides {
    /// `<provider>/<model>`, such as `local/qwen2.5:0.5b`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Submitted {
    /// Names this question in its `v1.intent.stream` notifications and in
    /// `v1.intent.cancel`.
    pub intent_id: String,
    /// Pass it back in `context` to ask a follow-up.
    pub conversation_id: String,
}

/// The params of a `v1.intent.stream` notification: one step of an answer.
/// Steps of one intent come in `seq` order: `started` once, `delta`s and
/// `usage`, then one of `done`, `error` or `cancelled`, the last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StreamStep {
    pub intent_id: String,
    /// From 1, with no gaps.
    pub seq: u64,
    pub kind: StepKind,
    /// `delta`: the new text. `done`: the whole answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// `started` and `done`: the model that answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `started`: the provider's name in settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// `started`: the model runs on this computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    /// `error`: what went wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

/// What a step of an answer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// The model took the question.
    Started,
    /// More text.
    Delta,
    /// Tokens used so far.
    Usage,
    /// The answer is whole.
    Done,
    /// No (whole) answer: see `error`.
    Error,
    /// Stopped by `v1.intent.cancel`.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    /// Input tokens the provider had cached, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached: Option<u64>,
}

/// `v1.intent.cancel`
pub struct Cancel;

impl Method for Cancel {
    const NAME: &'static str = "v1.intent.cancel";
    const SUMMARY: &'static str = "Stops an answer that is still being written.";
    const READ_ONLY: bool = false;
    type Params = CancelParams;
    type Result = Done;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CancelParams {
    /// From `v1.intent.submit`.
    pub intent_id: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // covers: CORE-04
    // suite: unit
    #[test]
    fn intent_steps_and_params_read_and_write_as_documented() {
        let params: SubmitParams = crate::methods::decode_params(Some(json!({
            "text": "what is btrfs?",
            "context": {"conversation_id": "c1", "surface": "bar"},
            "overrides": {"model": "local/qwen2.5:0.5b"}
        })))
        .unwrap();
        assert_eq!(
            params.context.unwrap().conversation_id.as_deref(),
            Some("c1")
        );
        assert_eq!(
            params.overrides.unwrap().model.as_deref(),
            Some("local/qwen2.5:0.5b")
        );
        let unknown =
            crate::methods::decode_params::<SubmitParams>(Some(json!({"text": "hi", "colour": 1})));
        assert!(unknown.is_err(), "params refuse unknown members");

        let step = StreamStep {
            intent_id: "i1".into(),
            seq: 2,
            kind: StepKind::Delta,
            text: Some("Hi".into()),
            model: None,
            provider: None,
            local: None,
            usage: None,
            error: None,
        };
        // Absent parts are left out, so the UI reads only what is there.
        assert_eq!(
            serde_json::to_value(&step).unwrap(),
            json!({"intent_id": "i1", "seq": 2, "kind": "delta", "text": "Hi"})
        );
        let usage: StreamStep = serde_json::from_value(json!({
            "intent_id": "i1", "seq": 3, "kind": "usage", "usage": {"input": 5, "output": 2}
        }))
        .unwrap();
        assert_eq!(usage.usage.unwrap().cached, None);
    }
}
