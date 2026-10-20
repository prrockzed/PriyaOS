//! The Phase 0 event payloads. Each is version 1 of its type. Fields are
//! closed (`deny_unknown_fields`) so a typo is caught when the event is
//! written, not when it is read years later.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::EventPayload;

/// Declares the wire type and version of a payload struct.
macro_rules! event {
    ($payload:ty, $type:literal, $version:literal) => {
        impl EventPayload for $payload {
            const TYPE: &'static str = $type;
            const VERSION: u16 = $version;
        }
    };
}

/// Where a settings change came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SettingSource {
    User,
    NaturalLanguage,
    Module,
    File,
    Rewind,
}

/// A setting got a new value (or lost one: `new` is empty). Settings values
/// are never `null`, so an empty `old`/`new` means "not set".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SettingChanged {
    #[serde(rename = "setting")]
    pub key: String,
    pub old: Option<Value>,
    pub new: Option<Value>,
    pub source: SettingSource,
}
event!(SettingChanged, "setting.changed", 2);

/// A settings file couldn't be read or didn't validate; the last good
/// settings stay in force.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SettingsLoadFailed {
    pub file: String,
    pub error: String,
}
event!(SettingsLoadFailed, "settings.load_failed", 1);

/// The user asked Priya something. The stream is `intent:<id>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentSubmitted {
    pub text: String,
    /// Groups follow-up questions with the one before.
    pub conversation: Option<String>,
    /// Where it was asked: `bar`, `cli`, …
    pub surface: String,
}
event!(IntentSubmitted, "intent.submitted", 1);

/// Priya's answer to an intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentAnswered {
    /// Long answers are moved to the blob store by the event store.
    pub answer: String,
    pub model: String,
}
event!(IntentAnswered, "intent.answered", 1);

/// The intent was cancelled before an answer arrived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentCancelled {
    pub reason: Option<String>,
}
event!(IntentCancelled, "intent.cancelled", 1);

/// How a model call ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallOutcome {
    Ok,
    Error,
    Cancelled,
}

/// One call to a model: what it cost and how long it took.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelCalled {
    pub provider: String,
    pub model: String,
    /// What the call was for, such as `answer` or `classify`.
    pub role: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub tokens_cached: u64,
    /// In millionths of a US dollar, so sums are exact. (Like every `u64`
    /// here, values above 2^53 would lose precision in JavaScript clients;
    /// that is $9 billion.)
    pub cost_micro_usd: u64,
    pub latency_ms: u64,
    pub outcome: CallOutcome,
    /// What went wrong, when `outcome` is `error`.
    pub error: Option<String>,
}
event!(ModelCalled, "model.called", 1);

/// A module process started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModuleStarted {
    pub module: String,
    pub version: String,
    pub pid: Option<u32>,
}
event!(ModuleStarted, "module.started", 1);

/// A module stopped on purpose (shutdown, disabled, upgraded).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModuleStopped {
    pub module: String,
    pub reason: String,
}
event!(ModuleStopped, "module.stopped", 1);

/// A module failed: it exited unexpectedly, stopped answering, broke the
/// protocol or couldn't start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModuleCrashed {
    pub module: String,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    /// How many times the host has restarted it so far.
    pub restarts: u32,
    /// What went wrong, when it didn't just exit.
    pub error: Option<String>,
    /// The last lines it wrote to stderr: at most 20, each cut to 300
    /// characters.
    pub stderr_tail: Option<String>,
    /// True when the host has stopped restarting it.
    pub gave_up: bool,
}
event!(ModuleCrashed, "module.crashed", 1);

/// A module's own event, namespaced by the module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModuleEvent {
    pub module: String,
    /// The module's event name, such as `forecast.updated`.
    pub name: String,
    pub data: Value,
}
event!(ModuleEvent, "module.event", 1);

/// priyad started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DaemonStarted {
    pub version: String,
    pub pid: u32,
}
event!(DaemonStarted, "daemon.started", 1);

/// priyad stopped cleanly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DaemonStopped {
    pub reason: String,
}
event!(DaemonStopped, "daemon.stopped", 1);
