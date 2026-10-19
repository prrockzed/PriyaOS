//! Events: the facts PriyaOS records in its append-only log (sub-phase 0.5,
//! CORE-01; design in docs/claude/architecture/02-core.md §1).
//!
//! Every event has a type and a schema version (`setting.changed` v1). Its
//! payload is a Rust struct registered in [`REGISTRY`], which is the only
//! list of what may be written: the store rejects unknown types and payloads
//! that don't match their struct. Older versions are lifted to the current
//! one by [`upcast`] when read.

mod types;

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use time::OffsetDateTime;
use ulid::Ulid;

use crate::error::{ErrorKind, UserFacing};
pub use types::*;

/// A recorded event: what happened, when, to which stream and by whom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventEnvelope {
    #[schemars(with = "String")]
    pub id: Ulid,
    /// UTC, written as RFC 3339.
    #[serde(with = "time::serde::rfc3339")]
    #[schemars(with = "String")]
    pub ts: OffsetDateTime,
    #[serde(rename = "type")]
    pub event_type: String,
    pub version: u16,
    /// The aggregate the event belongs to, such as `intent:01J…`.
    pub stream: String,
    /// The task or conversation the event is part of.
    pub correlation: Option<String>,
    /// The event that caused this one.
    #[schemars(with = "Option<String>")]
    pub causation: Option<Ulid>,
    pub actor: Actor,
    pub payload: Value,
}

/// An event to append. The store gives it an id and a timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NewEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub version: u16,
    pub stream: String,
    pub correlation: Option<String>,
    #[schemars(with = "Option<String>")]
    pub causation: Option<Ulid>,
    pub actor: Actor,
    pub payload: Value,
}

impl NewEvent {
    /// An event carrying `payload`, with its type and version taken from the
    /// payload's struct.
    pub fn new<P: EventPayload>(stream: impl Into<String>, actor: Actor, payload: &P) -> Self {
        Self {
            event_type: P::TYPE.to_owned(),
            version: P::VERSION,
            stream: stream.into(),
            correlation: None,
            causation: None,
            actor,
            // A payload struct always serialises; `Null` would fail validation.
            payload: serde_json::to_value(payload).unwrap_or(Value::Null),
        }
    }

    pub fn correlated(mut self, correlation: impl Into<String>) -> Self {
        self.correlation = Some(correlation.into());
        self
    }

    pub fn caused_by(mut self, event: Ulid) -> Self {
        self.causation = Some(event);
        self
    }
}

/// Who did it. Written as one string: `user`, `priya-ai`, `agent:<name>`,
/// `module:<name>`, `policy` or `system` (priyad itself).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Actor {
    User,
    PriyaAi,
    Agent(String),
    Module(String),
    Policy,
    System,
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => f.write_str("user"),
            Self::PriyaAi => f.write_str("priya-ai"),
            Self::Agent(name) => write!(f, "agent:{name}"),
            Self::Module(name) => write!(f, "module:{name}"),
            Self::Policy => f.write_str("policy"),
            Self::System => f.write_str("system"),
        }
    }
}

impl FromStr for Actor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let named = |name: &str, make: fn(String) -> Self| {
            if name.is_empty() {
                Err(format!("`{s}` needs a name after the colon"))
            } else {
                Ok(make(name.to_owned()))
            }
        };
        match s {
            "user" => Ok(Self::User),
            "priya-ai" => Ok(Self::PriyaAi),
            "policy" => Ok(Self::Policy),
            "system" => Ok(Self::System),
            _ => match s.split_once(':') {
                Some(("agent", name)) => named(name, Self::Agent),
                Some(("module", name)) => named(name, Self::Module),
                _ => Err(format!("`{s}` is not an actor")),
            },
        }
    }
}

impl Serialize for Actor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Actor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Actor {
    fn schema_name() -> Cow<'static, str> {
        "Actor".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^(user|priya-ai|policy|system|agent:.+|module:.+)$"
        })
    }
}

/// A payload struct. Its `TYPE` and `VERSION` identify it on the wire.
pub trait EventPayload: Serialize + DeserializeOwned + JsonSchema {
    const TYPE: &'static str;
    const VERSION: u16;
}

/// One registered `(type, version)`.
#[derive(Debug, Clone, Copy)]
pub struct EventKind {
    pub event_type: &'static str,
    pub version: u16,
    check: fn(&Value) -> Result<(), String>,
    schema: fn() -> Schema,
}

impl EventKind {
    pub const fn of<P: EventPayload>() -> Self {
        Self {
            event_type: P::TYPE,
            version: P::VERSION,
            check: check_as::<P>,
            schema: schema_of::<P>,
        }
    }

    /// The payload matches the struct exactly (unknown fields fail too).
    pub fn validate(&self, payload: &Value) -> Result<(), EventError> {
        (self.check)(payload).map_err(|reason| EventError::InvalidPayload {
            event_type: self.event_type.to_owned(),
            version: self.version,
            reason,
        })
    }

    /// The payload's JSON Schema, exported for clients in 0.9.
    pub fn schema(&self) -> Schema {
        (self.schema)()
    }
}

fn check_as<P: EventPayload>(payload: &Value) -> Result<(), String> {
    P::deserialize(payload)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn schema_of<P: EventPayload>() -> Schema {
    schemars::schema_for!(P)
}

/// Every event type PriyaOS may write. A new type or version is added here,
/// in the same PR as its struct and a line in 02-core.md §1.1.
pub static REGISTRY: &[EventKind] = &[
    EventKind::of::<SettingChanged>(),
    EventKind::of::<SettingsLoadFailed>(),
    EventKind::of::<IntentSubmitted>(),
    EventKind::of::<IntentAnswered>(),
    EventKind::of::<IntentCancelled>(),
    EventKind::of::<ModelCalled>(),
    EventKind::of::<ModuleStarted>(),
    EventKind::of::<ModuleStopped>(),
    EventKind::of::<ModuleCrashed>(),
    EventKind::of::<ModuleEvent>(),
    EventKind::of::<DaemonStarted>(),
    EventKind::of::<DaemonStopped>(),
];

/// The registered `(type, version)`, if any.
pub fn kind(event_type: &str, version: u16) -> Option<&'static EventKind> {
    REGISTRY
        .iter()
        .find(|k| k.event_type == event_type && k.version == version)
}

/// The newest registered version of a type.
pub fn current(event_type: &str) -> Option<&'static EventKind> {
    REGISTRY
        .iter()
        .filter(|k| k.event_type == event_type)
        .max_by_key(|k| k.version)
}

/// Lifts a payload from `from` to `from + 1`. Added when a type gets a new
/// version, so old events still read as the current struct.
#[derive(Debug, Clone, Copy)]
pub struct Upcaster {
    pub event_type: &'static str,
    pub from: u16,
    pub apply: fn(Value) -> Value,
}

/// No type has a second version yet. 0.9's schema checks require an entry
/// here for every version bump.
pub static UPCASTERS: &[Upcaster] = &[];

/// The payload lifted to the type's current version, with that version.
pub fn upcast(event_type: &str, version: u16, payload: Value) -> Result<(u16, Value), EventError> {
    upcast_with(UPCASTERS, event_type, version, payload)
}

fn upcast_with(
    upcasters: &[Upcaster],
    event_type: &str,
    mut version: u16,
    mut payload: Value,
) -> Result<(u16, Value), EventError> {
    let unknown = || EventError::UnknownType {
        event_type: event_type.to_owned(),
        version,
    };
    let target = current(event_type).ok_or_else(unknown)?.version;
    if version > target {
        return Err(unknown());
    }
    while version < target {
        let step = upcasters
            .iter()
            .find(|u| u.event_type == event_type && u.from == version)
            .ok_or_else(|| EventError::NoUpcaster {
                event_type: event_type.to_owned(),
                from: version,
            })?;
        payload = (step.apply)(payload);
        version += 1;
    }
    Ok((version, payload))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventError {
    #[error("unknown event type {event_type} v{version}")]
    UnknownType { event_type: String, version: u16 },
    #[error("{event_type} v{version} payload is invalid: {reason}")]
    InvalidPayload {
        event_type: String,
        version: u16,
        reason: String,
    },
    #[error("no upcaster for {event_type} from v{from}")]
    NoUpcaster { event_type: String, from: u16 },
}

impl UserFacing for EventError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::NoUpcaster { .. } => ErrorKind::Internal,
            _ => ErrorKind::InvalidInput,
        }
    }

    fn user_message(&self) -> String {
        match self {
            Self::UnknownType { .. } | Self::InvalidPayload { .. } => {
                "PriyaOS was asked to record something it doesn't recognise.".into()
            }
            Self::NoUpcaster { .. } => {
                "PriyaOS can't read an old record; please report this.".into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-01
    // suite: unit
    #[test]
    fn actors_are_one_string_on_the_wire() {
        for (actor, text) in [
            (Actor::User, "user"),
            (Actor::PriyaAi, "priya-ai"),
            (Actor::Agent("claude-code".into()), "agent:claude-code"),
            (Actor::Module("weather".into()), "module:weather"),
            (Actor::Policy, "policy"),
            (Actor::System, "system"),
        ] {
            assert_eq!(serde_json::to_value(&actor).unwrap(), Value::from(text));
            assert_eq!(serde_json::from_value::<Actor>(text.into()).unwrap(), actor);
        }
        for bad in ["root", "agent:", "module", ""] {
            assert!(bad.parse::<Actor>().is_err(), "{bad}");
        }
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn registry_has_each_type_once_and_validates_payloads() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in REGISTRY {
            assert!(
                seen.insert((kind.event_type, kind.version)),
                "{} twice",
                kind.event_type
            );
            assert!(kind.schema().as_object().is_some());
        }
        let event = NewEvent::new(
            "settings",
            Actor::User,
            &SettingChanged {
                key: "ai.strategy".into(),
                old: None,
                new: Some(Value::from("cheapest")),
                source: SettingSource::User,
            },
        );
        let kind = kind(&event.event_type, event.version).unwrap();
        kind.validate(&event.payload).unwrap();

        let mut extra = event.payload.clone();
        extra["surprise"] = Value::from(1);
        assert!(matches!(
            kind.validate(&extra),
            Err(EventError::InvalidPayload { .. })
        ));
        assert!(kind.validate(&Value::from("not an object")).is_err());
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn unknown_types_and_versions_are_not_registered() {
        assert!(
            super::kind("task.created", 1).is_none(),
            "not a Phase 0 type"
        );
        assert!(
            super::kind("setting.changed", 2).is_none(),
            "no such version"
        );
        let err = upcast("nope.nothing", 1, Value::Null).unwrap_err();
        assert!(matches!(err, EventError::UnknownType { .. }));
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn upcasting_lifts_old_payloads_step_by_step() {
        // The real registry has nothing to lift yet: current versions pass.
        let payload = serde_json::json!({"reason": "shutdown"});
        assert_eq!(
            upcast("daemon.stopped", 1, payload.clone()).unwrap(),
            (1, payload.clone())
        );
        assert!(
            upcast("daemon.stopped", 9, payload.clone()).is_err(),
            "newer than known"
        );

        // With a pretend v0 → v1 step, an old payload is lifted.
        let rename = Upcaster {
            event_type: "daemon.stopped",
            from: 0,
            apply: |mut v| {
                if let Some(why) = v.as_object_mut().and_then(|o| o.remove("why")) {
                    v["reason"] = why;
                }
                v
            },
        };
        let old = serde_json::json!({"why": "shutdown"});
        assert_eq!(
            upcast_with(&[rename], "daemon.stopped", 0, old.clone()).unwrap(),
            (1, payload)
        );
        assert!(matches!(
            upcast_with(&[], "daemon.stopped", 0, old),
            Err(EventError::NoUpcaster { from: 0, .. })
        ));
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn new_events_carry_their_type_links_and_actor() {
        let cause = Ulid::from_parts(1, 2);
        let event = NewEvent::new(
            "intent:01",
            Actor::User,
            &IntentSubmitted {
                text: "what's using my disk?".into(),
                conversation: None,
                surface: "bar".into(),
            },
        )
        .correlated("intent:01")
        .caused_by(cause);
        assert_eq!(event.event_type, "intent.submitted");
        assert_eq!(event.version, 1);
        assert_eq!(event.correlation.as_deref(), Some("intent:01"));
        assert_eq!(event.causation, Some(cause));

        // On the wire the type field is `type`, and it reads back unchanged.
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "intent.submitted");
        assert_eq!(serde_json::from_value::<NewEvent>(json).unwrap(), event);
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn schemas_describe_actor_and_envelope() {
        let actor = serde_json::to_value(schemars::schema_for!(Actor)).unwrap();
        assert_eq!(actor["type"], "string");
        assert!(actor["pattern"].as_str().unwrap().contains("module:"));
        let envelope = serde_json::to_value(schemars::schema_for!(EventEnvelope)).unwrap();
        for field in ["id", "ts", "type", "version", "stream", "actor", "payload"] {
            assert!(envelope["properties"].get(field).is_some(), "{field}");
        }
    }

    // covers: CORE-01
    // suite: unit
    #[test]
    fn event_errors_explain_themselves_plainly() {
        let invalid = EventError::InvalidPayload {
            event_type: "setting.changed".into(),
            version: 1,
            reason: "missing field `key`".into(),
        };
        assert_eq!(invalid.kind(), ErrorKind::InvalidInput);
        assert!(invalid.to_string().contains("missing field `key`"));
        assert!(
            !invalid.user_message().contains("setting.changed"),
            "no jargon"
        );

        let lost = EventError::NoUpcaster {
            event_type: "setting.changed".into(),
            from: 1,
        };
        assert_eq!(lost.kind(), ErrorKind::Internal);
        assert!(lost.user_message().contains("report"));
    }
}
