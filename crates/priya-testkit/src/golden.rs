//! Golden-file (snapshot) assertions over `insta`, with values that change
//! on every run replaced by placeholders: ULIDs become `[ulid]`, RFC 3339
//! timestamps `[timestamp]`, and durations such as `1.5s` or `20ms`
//! `[duration]`.
//!
//! ```ignore
//! priya_testkit::assert_golden_json!("event_appended", event);
//! priya_testkit::assert_golden_text!("doctor_report", output);
//! ```
//!
//! Snapshots live next to the calling test, in `snapshots/`; review new ones
//! with `cargo insta review`.

use serde::Serialize;
use serde_json::Value;

/// `value` as pretty JSON with every string redacted.
pub fn json(value: &impl Serialize) -> String {
    let value = serde_json::to_value(value).unwrap_or_else(|e| Value::String(format!("<{e}>")));
    let redacted = redact_value(value);
    serde_json::to_string_pretty(&redacted).unwrap_or_else(|e| format!("<{e}>"))
}

/// `text` with ULIDs, timestamps and durations replaced.
pub fn text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut previous: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        // Matches start only at a word boundary.
        let at_boundary = previous.is_none_or(|p| !p.is_ascii_alphanumeric());
        let matched = at_boundary
            .then(|| {
                [
                    (timestamp_len(rest), "[timestamp]"),
                    (ulid_len(rest), "[ulid]"),
                    (duration_len(rest), "[duration]"),
                ]
                .into_iter()
                .find_map(|(len, placeholder)| len.map(|len| (len, placeholder)))
            })
            .flatten();
        match matched {
            Some((len, placeholder)) => {
                out.push_str(placeholder);
                previous = rest[..len].chars().last();
                rest = &rest[len..];
            }
            None => {
                out.push(c);
                previous = Some(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

fn redact_value(value: Value) -> Value {
    match value {
        Value::String(s) => Value::String(text(&s)),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_value).collect()),
        Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, redact_value(v))).collect())
        }
        other => other,
    }
}

/// Snapshot of `value` as redacted JSON.
#[macro_export]
macro_rules! assert_golden_json {
    ($name:expr, $value:expr $(,)?) => {
        $crate::insta::assert_snapshot!($name, $crate::golden::json(&$value))
    };
}

/// Snapshot of `text` with ULIDs, timestamps and durations redacted.
#[macro_export]
macro_rules! assert_golden_text {
    ($name:expr, $text:expr $(,)?) => {
        $crate::insta::assert_snapshot!($name, $crate::golden::text(&$text))
    };
}

/// A ULID: 26 Crockford base32 characters (no I, L, O, U), not followed by
/// another letter or digit.
fn ulid_len(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let is_crockford =
        |b: u8| b.is_ascii_digit() || (b.is_ascii_uppercase() && !b"ILOU".contains(&b));
    (bytes.len() >= 26
        && bytes[..26].iter().all(|b| is_crockford(*b))
        && bytes[..26].iter().any(u8::is_ascii_digit)
        && ends_word(bytes, 26))
    .then_some(26)
}

/// `2026-10-20T03:04:05Z`, with optional fractional seconds and a `Z` or
/// `±HH:MM` offset.
fn timestamp_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let digits = |from: usize, n: usize| {
        b.len() >= from + n && b[from..from + n].iter().all(u8::is_ascii_digit)
    };
    let at = |i: usize, c: u8| b.get(i) == Some(&c);
    if !(digits(0, 4)
        && at(4, b'-')
        && digits(5, 2)
        && at(7, b'-')
        && digits(8, 2)
        && (at(10, b'T') || at(10, b' '))
        && digits(11, 2)
        && at(13, b':')
        && digits(14, 2)
        && at(16, b':')
        && digits(17, 2))
    {
        return None;
    }
    let mut i = 19;
    if at(i, b'.') {
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    if at(i, b'Z') {
        i += 1;
    } else if (at(i, b'+') || at(i, b'-'))
        && digits(i + 1, 2)
        && at(i + 3, b':')
        && digits(i + 4, 2)
    {
        i += 6;
    }
    ends_word(b, i).then_some(i)
}

/// A number with a time unit: `20ms`, `1.5s`, `3m`, `250µs`.
fn duration_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    if b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    let rest = &s[i..];
    ["ns", "µs", "us", "ms", "s", "m", "h"]
        .iter()
        .find(|unit| rest.starts_with(*unit) && ends_word(b, i + unit.len()))
        .map(|unit| i + unit.len())
}

/// True when byte `i` doesn't continue a word.
fn ends_word(bytes: &[u8], i: usize) -> bool {
    bytes.get(i).is_none_or(|b| !b.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-04
    // suite: unit
    // acceptance: 0.4
    #[test]
    fn golden_redacts_ulids_and_timestamps() {
        assert_eq!(
            text(
                "task 01JABCDEFGHJKMNPQRSTVWXYZ0 at 2026-10-20T03:04:05.123Z took 1.5s (and 20ms)"
            ),
            "task [ulid] at [timestamp] took [duration] (and [duration])"
        );
        assert_eq!(text("at 2026-10-20 03:04:05+05:30."), "at [timestamp].");
        // Ordinary words and numbers stay.
        assert_eq!(
            text("3 files, 5 seconds, ms, 12x"),
            "3 files, 5 seconds, ms, 12x"
        );
        assert_eq!(
            text("ABCDEFGHJKMNPQRSTVWXYZABCD"),
            "ABCDEFGHJKMNPQRSTVWXYZABCD",
            "no digit, not a ULID"
        );

        let value = serde_json::json!({
            "id": "01JABCDEFGHJKMNPQRSTVWXYZ0",
            "at": "2026-10-20T03:04:05Z",
            "steps": [{"took": "250ms", "count": 3}],
        });
        crate::assert_golden_json!("redacted_event", value);
    }
}
