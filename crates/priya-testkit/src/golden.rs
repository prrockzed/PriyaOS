//! Golden-file (snapshot) assertions over `insta`, with values that change
//! on every run replaced by placeholders: ULIDs become `[ulid]`, RFC 3339
//! timestamps `[timestamp]`, and durations `[duration]`. A duration is a
//! number with `ns`, `µs`, `us` or `ms`, or a decimal number with `s`, `m`
//! or `h` (`1.5s`); plain `5m` or `12h` are left alone, since in prose they
//! are often not durations.
//!
//! Only string values are redacted, not object keys: redacting keys could
//! merge entries. Key maps by something stable in tests.
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

/// `value` as pretty JSON with every string redacted. Panics, failing the
/// test, when `value` can't be serialised.
#[track_caller]
pub fn json(value: &impl Serialize) -> String {
    let value = match serde_json::to_value(value) {
        Ok(value) => value,
        Err(e) => panic!("golden::json: the value can't be serialised: {e}"),
    };
    match serde_json::to_string_pretty(&redact_value(value)) {
        Ok(text) => text,
        Err(e) => panic!("golden::json: the redacted value can't be printed: {e}"),
    }
}

/// `text` with ULIDs, timestamps and durations replaced.
pub fn text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut previous: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        // Matches start only at a word boundary.
        let at_boundary = previous.is_none_or(|p| !is_word_char(p));
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
        $crate::insta::assert_snapshot!($name, $crate::golden::json(&$value), stringify!($value))
    };
}

/// Snapshot of `text` with ULIDs, timestamps and durations redacted.
#[macro_export]
macro_rules! assert_golden_text {
    ($name:expr, $text:expr $(,)?) => {
        $crate::insta::assert_snapshot!($name, $crate::golden::text(&$text), stringify!($text))
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

/// `2026-10-20T03:04:05Z`, with `T` or a space (either case), optional
/// fractional seconds, and an optional `Z`, `±HH:MM` or `±HH` offset.
fn timestamp_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let digits = |from: usize, n: usize| {
        b.len() >= from + n && b[from..from + n].iter().all(u8::is_ascii_digit)
    };
    let at = |i: usize, c: u8| b.get(i) == Some(&c);
    let date_and_time = digits(0, 4)
        && at(4, b'-')
        && digits(5, 2)
        && at(7, b'-')
        && digits(8, 2)
        && (at(10, b'T') || at(10, b't') || at(10, b' '))
        && digits(11, 2)
        && at(13, b':')
        && digits(14, 2)
        && at(16, b':')
        && digits(17, 2);
    if !date_and_time {
        return None;
    }
    let mut i = 19;
    // A fraction only when digits follow, so a full stop stays put.
    if at(i, b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    if at(i, b'Z') || at(i, b'z') {
        i += 1;
    } else if (at(i, b'+') || at(i, b'-')) && digits(i + 1, 2) {
        i += 3;
        if at(i, b':') && digits(i + 1, 2) {
            i += 3;
        }
    }
    ends_word(b, i).then_some(i)
}

/// `20ms`, `250µs`, `3ns`, or a decimal with `s`, `m` or `h` (`1.5s`).
fn duration_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let count_digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut i = count_digits(0);
    if i == 0 {
        return None;
    }
    let mut fraction = false;
    if b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
        i += 1 + count_digits(i + 1);
        fraction = true;
    }
    let rest = &s[i..];
    let units: &[&str] = if fraction {
        &["ns", "µs", "us", "ms", "s", "m", "h"]
    } else {
        &["ns", "µs", "us", "ms"]
    };
    units
        .iter()
        .find(|unit| rest.starts_with(*unit) && ends_word(b, i + unit.len()))
        .map(|unit| i + unit.len())
}

/// Letters, digits and `_` continue a word.
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// True when byte `i` doesn't continue a word.
fn ends_word(bytes: &[u8], i: usize) -> bool {
    bytes.get(i).is_none_or(|b| !is_word_char(char::from(*b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-04
    // suite: unit
    // acceptance: 0.4
    // regression: 0.4
    #[test]
    fn golden_redacts_ulids_and_timestamps() {
        assert_eq!(
            text(
                "task 01JABCDEFGHJKMNPQRSTVWXYZ0 at 2026-10-20T03:04:05.123Z took 1.5s (and 20ms)"
            ),
            "task [ulid] at [timestamp] took [duration] (and [duration])"
        );
        assert_eq!(text("at 2026-10-20 03:04:05+05:30."), "at [timestamp].");
        assert_eq!(
            text("at 2026-10-20 03:04:05."),
            "at [timestamp].",
            "the full stop stays"
        );
        assert_eq!(
            text("2026-10-20t03:04:05z and 2026-10-20T03:04:05+05"),
            "[timestamp] and [timestamp]"
        );
        // Prose numbers with units are not durations; decimals and ms are.
        assert_eq!(
            text("5m tall, 1s and 0s, 12h ago, 10m users"),
            "5m tall, 1s and 0s, 12h ago, 10m users"
        );
        assert_eq!(
            text("took 3.25s, 40µs, 7ns"),
            "took [duration], [duration], [duration]"
        );
        assert_eq!(text("1_000ms"), "1_000ms", "`_` continues the word");
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
