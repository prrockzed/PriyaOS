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
    type Params = NoParams;
    type Result = StatusResult;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StatusResult {
    /// The daemon's version.
    pub version: String,
    pub uptime_seconds: u64,
    /// Where the event log is.
    pub db_path: String,
    pub event_count: u64,
    /// Running modules, by name (from 0.8).
    pub modules: Vec<String>,
    /// The model answering, as `fixed:<provider>/<model>`, if one is set.
    pub configured_model: Option<String>,
}

/// `v1.system.methods`
pub struct Methods;

impl Method for Methods {
    const NAME: &'static str = "v1.system.methods";
    const SUMMARY: &'static str = "Lists every request the API accepts.";
    type Params = NoParams;
    type Result = MethodsResult;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MethodsResult {
    pub methods: Vec<MethodDescription>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MethodDescription {
    pub name: String,
    pub summary: String,
    pub streams: bool,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FixMode {
    DryRun,
    Apply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorReport {
    pub checks: Vec<CheckResult>,
    /// The ids of the checks whose fixes were made (with `fix: apply`).
    pub fixed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    /// Stable id, such as `store.integrity`.
    pub id: String,
    pub title: String,
    pub status: CheckStatus,
    /// One plain sentence.
    pub summary: String,
    /// Technical detail, behind "Details".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixInfo>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Info,
    Warn,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FixScope {
    User,
    System,
}
