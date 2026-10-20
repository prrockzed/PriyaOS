//! `v1.events.*`: following the event log (02-core §1.3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Done, Method};
use crate::events::EventEnvelope;

/// The notification carrying each event of a subscription.
pub const EVENT_NOTIFICATION: &str = "v1.events.event";
/// The notification that a subscription stopped without being asked to.
pub const ENDED_NOTIFICATION: &str = "v1.events.ended";

/// `v1.events.subscribe`
pub struct Subscribe;

impl Method for Subscribe {
    const NAME: &'static str = "v1.events.subscribe";
    const SUMMARY: &'static str = "Sends events as they happen, from a point in the log on.";
    const READ_ONLY: bool = true;
    const NOTIFICATION: Option<&'static str> = Some(EVENT_NOTIFICATION);
    type Params = SubscribeParams;
    type Result = Subscription;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubscribeParams {
    /// Type patterns, `*` matching anything (`intent.*`). All when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    /// Only streams starting with this, such as `intent:`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_prefix: Option<String>,
    /// Events after this seq; from now on when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_seq: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Subscription {
    /// Names the subscription in its notifications and in `unsubscribe`.
    pub subscription: String,
}

/// The params of a `v1.events.event` notification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventNotification {
    /// The subscription it belongs to.
    pub subscription: String,
    pub event: StoredEvent,
}

/// The params of a `v1.events.ended` notification: the subscription sends
/// nothing more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EndedNotification {
    pub subscription: String,
    pub reason: EndReason,
    /// What happened, in plain words.
    pub message: String,
}

/// Why a subscription stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// The daemon is stopping.
    Cancelled,
    /// Something went wrong reading the history.
    Error,
}

/// An event as stored: its place in the log, the event, and the hashes that
/// chain it to the one before. priyad sends the full payload; converted
/// straight from the store, a payload over 8 KiB is still the link
/// `{"$blob": …, "size": n}` to its blob.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StoredEvent {
    /// Its place in the log, from 1.
    pub seq: i64,
    pub event: EventEnvelope,
    /// The hash of the event before it.
    pub prev_hash: String,
    /// This event's hash.
    pub hash: String,
}

/// `v1.events.unsubscribe`
pub struct Unsubscribe;

impl Method for Unsubscribe {
    const NAME: &'static str = "v1.events.unsubscribe";
    const SUMMARY: &'static str = "Stops a subscription.";
    const READ_ONLY: bool = true;
    type Params = UnsubscribeParams;
    type Result = Done;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnsubscribeParams {
    /// The id `v1.events.subscribe` gave.
    pub subscription: String,
}
