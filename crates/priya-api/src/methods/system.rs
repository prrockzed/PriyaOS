//! `v1.system.*`: the daemon itself.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Method, NoParams};

/// `v1.system.status`
pub struct Status;

impl Method for Status {
    const NAME: &'static str = "v1.system.status";
    const SUMMARY: &'static str = "Shows whether PriyaOS is running and what it is using.";
    const READ_ONLY: bool = true;
    type Params = NoParams;
    type Result = StatusResult;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StatusResult {
    /// The daemon's version.
    pub version: String,
    /// How long it has been running, in seconds.
    pub uptime_seconds: u64,
    /// Where the event log is.
    pub db_path: String,
    /// How many events the log holds.
    pub event_count: u64,
    /// Running modules, by name (from 0.8).
    pub modules: Vec<String>,
    /// The model that answers, if one is set up.
    pub model: Option<ConfiguredModel>,
}

/// A model, and the provider that serves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConfiguredModel {
    /// The provider's name in your settings, such as `anthropic`.
    pub provider: String,
    /// The model's name at that provider.
    pub model: String,
}

/// `v1.system.methods`
pub struct Methods;

impl Method for Methods {
    const NAME: &'static str = "v1.system.methods";
    const SUMMARY: &'static str = "Lists every request the API accepts.";
    const READ_ONLY: bool = true;
    type Params = NoParams;
    type Result = MethodsResult;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MethodsResult {
    pub methods: Vec<MethodDescription>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MethodDescription {
    /// The method's name, such as `v1.system.status`.
    pub name: String,
    /// What it does, in one sentence.
    pub summary: String,
    /// Whether it changes nothing.
    pub read_only: bool,
    /// The notification it sends, if it is a subscription.
    pub notification: Option<String>,
    /// JSON Schema of the params.
    pub params: Value,
    /// JSON Schema of the result.
    pub result: Value,
}

/// `v1.system.doctor`
pub struct Doctor;

impl Method for Doctor {
    const NAME: &'static str = "v1.system.doctor";
    const SUMMARY: &'static str = "Checks that PriyaOS is healthy, and can fix what it finds.";
    // With `fix: apply` it changes things.
    const READ_ONLY: bool = false;
    type Params = DoctorParams;
    type Result = DoctorReport;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DoctorParams {
    /// List the fixes (`dry_run`) or make them (`apply`). Without it, only
    /// checks run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixMode>,
    /// The asking command's version, to check it matches the daemon's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_version: Option<String>,
}

/// What to do with the fixes the checks offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FixMode {
    /// List them without changing anything.
    DryRun,
    /// Make them.
    Apply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorReport {
    /// Every check, in a fixed order.
    pub checks: Vec<CheckResult>,
    /// The ids of the checks whose fixes were made (with `fix: apply`).
    pub fixed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    /// Stable id, such as `store.integrity`.
    pub id: String,
    /// What it checks, in a few words.
    pub title: String,
    /// How it went.
    pub status: CheckStatus,
    /// One plain sentence.
    pub summary: String,
    /// Technical detail, behind "Details".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixInfo>,
}

/// How a check went, from best to worst.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// All good.
    Ok,
    /// Worth knowing, nothing to do.
    Info,
    /// Works, but needs attention.
    Warn,
    /// Broken.
    Fail,
}

/// A fix a check offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FixInfo {
    /// `user` fixes touch only your own files; `system` ones need the root
    /// helper (from 2.4).
    pub scope: FixScope,
    /// What the fix does, in one plain sentence.
    pub description: String,
}

/// Whose files a fix changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FixScope {
    /// Only your own files.
    User,
    /// System files, through the root helper (from 2.4).
    System,
}
