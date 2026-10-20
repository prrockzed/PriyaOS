//! `v1.events.*`: following the event log (02-core §1.3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Done, Method};
use crate::events::EventEnvelope;

/// The notification carrying each event of a subscription.
pub const EVENT_NOTIFICATION: &str = "v1.events.event";

/// `v1.events.subscribe`
pub struct Subscribe;

impl Method for Subscribe {
    const NAME: &'static str = "v1.events.subscribe";
    const SUMMARY: &'static str = "Sends events as they happen, from a point in the log on.";
    const STREAMS: bool = true;
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
    pub subscription: String,
    pub event: StoredEvent,
}

/// An event as stored: its place in the log, the event, and its hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StoredEvent {
    pub seq: i64,
    pub event: EventEnvelope,
    pub hash: String,
}

/// `v1.events.unsubscribe`
pub struct Unsubscribe;

impl Method for Unsubscribe {
    const NAME: &'static str = "v1.events.unsubscribe";
    const SUMMARY: &'static str = "Stops a subscription.";
    type Params = Subscription;
    type Result = Done;
}
