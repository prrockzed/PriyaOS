//! [`assert_events!`]: check which events were recorded, in order, and what
//! their payloads say.
//!
//! ```ignore
//! let records = store.read_after(0, 100).await?;
//! priya_testkit::assert_events!(
//!     records.iter().map(|r| &r.event),
//!     [
//!         "intent.submitted" => {"text": "what's using my disk?"},
//!         "model.called",
//!         "intent.answered",
//!     ]
//! );
//! ```
//!
//! A payload matcher is a JSON pattern: every key it names must be present
//! with that value (nested objects match the same way); keys it leaves out
//! are ignored. A matcher is one token tree: an object, an array, a string,
//! `true`, `null` or a number; wrap a negative number in parentheses.
//!
//! It works on envelopes, so it suits any source of events. Patterns see the
//! payload as given: a stored event over 8 KiB holds only
//! `{"$blob": …, "size": n}`, so resolve it with `EventStore::payload` first.
//! (The testkit can't name the store: that would be a dependency cycle.)

use priya_api::events::{Actor, EventEnvelope, NewEvent, SettingChanged, SettingSource};
use serde_json::Value;

/// A valid event for tests that need many: `setting.changed` of `key` to
/// `value`, from a settings file.
pub fn setting_event(key: &str, value: Value) -> NewEvent {
    NewEvent::new(
        "settings",
        Actor::System,
        &SettingChanged {
            key: key.into(),
            old: None,
            new: Some(value),
            source: SettingSource::File,
        },
    )
    .expect("a SettingChanged always serialises")
}

/// One expected event: its type, and optionally a payload pattern.
#[derive(Debug, Clone)]
pub struct Expected {
    pub event_type: &'static str,
    pub payload: Option<Value>,
}

/// Panics with a readable listing unless `actual` matches `expected`
/// exactly in number and order.
#[track_caller]
pub fn assert_events<'a>(
    actual: impl IntoIterator<Item = &'a EventEnvelope>,
    expected: &[Expected],
) {
    let actual: Vec<&EventEnvelope> = actual.into_iter().collect();
    let mut problems = Vec::new();
    if actual.len() != expected.len() {
        problems.push(format!(
            "expected {}, got {}",
            count(expected.len()),
            actual.len()
        ));
    }
    for (i, (event, want)) in actual.iter().zip(expected).enumerate() {
        if event.event_type != want.event_type {
            problems.push(format!(
                "event {i}: expected {}, got {}",
                want.event_type, event.event_type
            ));
        } else if let Some(pattern) = &want.payload
            && !matches(pattern, &event.payload)
        {
            problems.push(format!(
                "event {i} ({}): payload {} doesn't match {pattern}",
                event.event_type, event.payload
            ));
        }
    }
    if !problems.is_empty() {
        let got: Vec<&str> = actual.iter().map(|e| e.event_type.as_str()).collect();
        panic!(
            "events don't match:\n  {}\nrecorded: {got:?}",
            problems.join("\n  ")
        );
    }
}

fn count(n: usize) -> String {
    if n == 1 {
        "1 event".into()
    } else {
        format!("{n} events")
    }
}

/// `pattern` is contained in `value`: objects by key, everything else equal.
fn matches(pattern: &Value, value: &Value) -> bool {
    match (pattern, value) {
        (Value::Object(want), Value::Object(have)) => want
            .iter()
            .all(|(key, w)| have.get(key).is_some_and(|h| matches(w, h))),
        _ => pattern == value,
    }
}

/// See the [module docs](crate::events).
#[macro_export]
macro_rules! assert_events {
    ($events:expr, [$($event_type:literal $(=> $payload:tt)?),* $(,)?]) => {
        $crate::events::assert_events(
            $events,
            &[$($crate::events::Expected {
                event_type: $event_type,
                payload: $crate::__payload_pattern!($($payload)?),
            }),*],
        )
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __payload_pattern {
    () => {
        ::core::option::Option::None
    };
    ($payload:tt) => {
        ::core::option::Option::Some($crate::serde_json::json!($payload))
    };
}

#[cfg(test)]
mod tests {
    use priya_api::events::Actor;
    use serde_json::json;
    use time::OffsetDateTime;
    use ulid::Ulid;

    use super::*;

    fn envelope(event_type: &str, payload: Value) -> EventEnvelope {
        EventEnvelope {
            id: Ulid::nil(),
            ts: OffsetDateTime::UNIX_EPOCH,
            event_type: event_type.into(),
            version: 1,
            stream: "s".into(),
            correlation: None,
            causation: None,
            actor: Actor::System,
            payload,
        }
    }

    fn recorded() -> Vec<EventEnvelope> {
        vec![
            envelope(
                "setting.changed",
                json!({"key": "ai.strategy", "new": "cheapest", "source": "user"}),
            ),
            envelope(
                "intent.submitted",
                json!({"text": "hi", "surface": "bar", "context": {"app": "kitty", "pid": 7}}),
            ),
        ]
    }

    // covers: QA-04
    // suite: unit
    // acceptance: 0.5
    #[test]
    fn assert_events_matches_types_and_payload_patterns() {
        let log = recorded();
        crate::assert_events!(
            &log,
            [
                "setting.changed" => {"key": "ai.strategy", "source": "user"},
                "intent.submitted" => {"context": {"app": "kitty"}},
            ]
        );

        let failure = |f: fn(&[EventEnvelope])| {
            let log = recorded();
            let result = std::panic::catch_unwind(|| f(&log));
            *result.unwrap_err().downcast::<String>().unwrap()
        };
        let wrong_type =
            failure(|log| crate::assert_events!(log, ["setting.changed", "intent.answered"]));
        assert!(
            wrong_type.contains("event 1: expected intent.answered, got intent.submitted"),
            "{wrong_type}"
        );
        let wrong_value = failure(
            |log| crate::assert_events!(log, ["setting.changed" => {"new": "best"}, "intent.submitted"]),
        );
        assert!(wrong_value.contains("doesn't match"), "{wrong_value}");
        let too_few = failure(|log| crate::assert_events!(log, ["setting.changed"]));
        assert!(too_few.contains("expected 1 event, got 2"), "{too_few}");
        let missing_key = failure(
            |log| crate::assert_events!(log, ["setting.changed" => {"old": null}, "intent.submitted"]),
        );
        assert!(missing_key.contains("doesn't match"), "{missing_key}");
        let wrong_nested = failure(|log| {
            crate::assert_events!(
                log,
                ["setting.changed", "intent.submitted" => {"context": {"pid": 8}}]
            )
        });
        assert!(wrong_nested.contains("event 1"), "{wrong_nested}");
    }
}
