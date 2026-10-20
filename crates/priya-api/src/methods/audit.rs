//! `v1.audit.*`: proving the log wasn't changed (02-core §1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Method;

/// `v1.audit.verify`
pub struct Verify;

impl Method for Verify {
    const NAME: &'static str = "v1.audit.verify";
    const SUMMARY: &'static str =
        "Checks that no event in the log was changed, removed or reordered.";
    const READ_ONLY: bool = true;
    type Params = VerifyParams;
    type Result = VerifyReport;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifyParams {
    /// The first seq to check; the start of the log when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<i64>,
    /// The last seq to check; the end of the log when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerifyReport {
    /// Whether every event checked fits.
    pub ok: bool,
    /// Events checked before stopping.
    pub checked: u64,
    /// For a gap, the missing seq; otherwise the first event that doesn't fit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_bad_seq: Option<i64>,
    /// Why the chain broke there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<BreakReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BreakReason {
    /// A field of the event was changed.
    HashMismatch,
    /// The event doesn't link to the one before it.
    PrevLinkBroken,
    /// An event is missing.
    SeqGap,
}
