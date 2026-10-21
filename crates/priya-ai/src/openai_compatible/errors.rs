//! What an HTTP failure means, in a plain sentence that names the
//! provider, and the technical detail behind "Details" with any key
//! removed.

use std::error::Error;
use std::time::Duration;

use time::OffsetDateTime;
use time::macros::format_description;

use crate::error::{ProviderError, ProviderErrorKind};

/// How much of an error body is kept as detail.
const DETAIL_LIMIT: usize = 300;

/// What the failed request was for, which changes what a status means.
#[derive(Debug, Clone, Copy)]
pub enum Purpose<'a> {
    /// An answer from this model.
    Answer { model: &'a str },
    /// The health check.
    Health,
}

/// Who failed: how to name the provider and what to tell the user to do.
pub struct Provider<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub base_url: &'a str,
    pub has_key: bool,
}

/// The longest wait we believe: a bigger one is a server bug or a wrong
/// clock, and would overflow when added to an instant.
const LONGEST_WAIT: Duration = Duration::from_secs(24 * 60 * 60);
/// Waits longer than this are a used-up quota, not a busy moment.
const QUOTA_WAIT: Duration = Duration::from_secs(2 * 60);

/// The error for an HTTP status. `body` is the start of the response body
/// and `wait` what the provider asked us to wait.
pub fn status_error(
    provider: &Provider<'_>,
    status: u16,
    purpose: Purpose<'_>,
    body: &str,
    wait: Option<Duration>,
) -> ProviderError {
    use ProviderErrorKind as Kind;
    let Provider { id, base_url, .. } = *provider;
    let name = capital(provider.label);
    let model = match purpose {
        Purpose::Answer { model } => Some(model),
        Purpose::Health => None,
    };
    let refused_key = || {
        let message = if provider.has_key {
            format!("{name} refused the key. Store a new one with `priya ai key set {id}`.")
        } else {
            format!("{name} needs a key. Store one with `priya ai key set {id}`.")
        };
        (Kind::Auth, message)
    };
    // Gemini answers a bad key with 400, not 401.
    let bad_key =
        status == 400 && (body.contains("API_KEY_INVALID") || body.contains("API key not valid"));
    // A 404 from a wrong address is a web page, not an error about a model.
    let unknown_model = body.contains("model");
    // Gemini closes old models to new keys with a 404 that says so (and
    // not a web page that happens to say "no longer available").
    let closed_model = body.contains("no longer available to new users");
    // A thinking level the model doesn't take (Gemini: "Thinking level
    // MINIMAL is not supported for this model").
    let bad_effort = status == 400 && body.to_ascii_lowercase().contains("thinking level");
    let wrong_address = || {
        (
            Kind::BadRequest,
            format!("{name} isn't at {base_url}. Check its address in settings."),
        )
    };
    let (kind, message) = match (status, model) {
        _ if bad_key || status == 401 => refused_key(),
        (400, Some(model)) if bad_effort => (
            Kind::BadRequest,
            format!(
                "{name} won't think at the level in settings for {model}. Change it with `priya settings set ai.providers.{id}.reasoning_effort <level>` (low, medium, high or default)."
            ),
        ),
        (404, Some(model)) if closed_model => (
            Kind::BadRequest,
            format!(
                "{name} no longer offers {model} to new keys. Choose a newer model with `priya settings set ai.strategy fixed:{id}/{}`.",
                newer_model(id)
            ),
        ),
        (403, Some(model)) if provider.has_key => (
            Kind::Auth,
            format!("{name} won't let this key use {model}."),
        ),
        (403, _) => refused_key(),
        (402, _) => (
            Kind::BadRequest,
            format!("{name} says the account has no credit left."),
        ),
        (404, Some(model)) if unknown_model => (
            Kind::BadRequest,
            format!("{name} doesn't have the model {model}."),
        ),
        // Redirects aren't followed: a moved or wrong address shows here.
        (300..=399 | 404, _) => wrong_address(),
        (408, _) => (
            Kind::Network,
            format!("{name} didn't get the whole question in time."),
        ),
        (413, _) => (
            Kind::BadRequest,
            format!("The question is too long for {}.", provider.label),
        ),
        (429, _) => match wait.filter(|w| *w > QUOTA_WAIT) {
            Some(wait) => (
                Kind::RateLimited,
                format!(
                    "{name}'s free limit is used up for now. Try again in {}.",
                    plain_duration(wait)
                ),
            ),
            None => (
                Kind::RateLimited,
                format!("{name} is getting too many questions. Try again in a moment."),
            ),
        },
        // Gateways in front of a busy model answer 502 or 504; both are
        // worth the one retry.
        (502 | 503 | 504 | 529, _) => (Kind::Overloaded, format!("{name} is busy right now.")),
        (500..=599, _) => (Kind::Server, format!("{name} had a problem on its side.")),
        _ => (
            Kind::BadRequest,
            format!("{name} didn't accept the question."),
        ),
    };
    ProviderError::new(kind, message).with_retry_after(wait)
}

/// A model to suggest instead of a closed one: PriyaOS's current pick for
/// Gemini, else a placeholder.
fn newer_model(provider: &str) -> &'static str {
    match provider {
        "gemini" => crate::gateway::route::GEMINI_MODEL,
        _ => "<model>",
    }
}

/// "about 3 hours", "about 40 minutes": how long, in words.
fn plain_duration(wait: Duration) -> String {
    let minutes = wait.as_secs().div_ceil(60);
    match minutes {
        0..=1 => "about a minute".to_owned(),
        2..=89 => format!("about {minutes} minutes"),
        _ => format!("about {} hours", minutes.div_ceil(60)),
    }
}

/// How long the provider asks us to wait: `Retry-After` in seconds or as
/// an HTTP date, else Gemini's `retryDelay` in the body (which may have a
/// fraction, rounded up). At most a day.
pub fn retry_after(header: Option<&str>, body: &str, now: OffsetDateTime) -> Option<Duration> {
    let asked = header
        .map(str::trim)
        .and_then(|header| {
            if let Ok(seconds) = header.parse::<u64>() {
                return Some(Duration::from_secs(seconds));
            }
            let http_date = format_description!(
                "[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] GMT"
            );
            let at = time::PrimitiveDateTime::parse(header, http_date).ok()?;
            Some(Duration::try_from(at.assume_utc() - now).unwrap_or(Duration::ZERO))
        })
        .or_else(|| {
            // `"retryDelay": "33s"` or `"1.5s"`
            let after = body.split("\"retryDelay\"").nth(1)?;
            let number: String = after
                .trim_start_matches(|c: char| c == ':' || c == '"' || c.is_whitespace())
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let seconds: f64 = number.parse().ok()?;
            Duration::try_from_secs_f64(seconds.ceil()).ok()
        })?;
    Some(asked.min(LONGEST_WAIT))
}

/// "HTTP 400 Bad Request: <start of the body>", with the key removed
/// before the body is cut, so no part of it can be left.
pub fn body_detail(status: &str, body: &str, key: Option<&str>) -> String {
    format!("HTTP {status}: {}", excerpt(&redact(body, key)))
}

/// An error's message with its causes ("error sending request: connection
/// refused"), which reqwest's own message leaves out.
pub fn chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

/// `text` without the key, which a server or proxy may echo back.
pub fn redact(text: &str, key: Option<&str>) -> String {
    match key.filter(|k| !k.is_empty()) {
        Some(key) => text.replace(key, "[key]"),
        None => text.to_owned(),
    }
}

/// `label` at the start of a sentence.
pub fn capital(label: &str) -> String {
    let mut chars = label.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The start of an error body, on one line, for details.
fn excerpt(body: &str) -> String {
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(DETAIL_LIMIT) {
        Some((cut, _)) => format!("{}…", &flat[..cut]),
        None => flat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEMINI: Provider<'static> = Provider {
        id: "gemini",
        label: "Gemini",
        base_url: "https://example.invalid/v1",
        has_key: true,
    };
    const ANSWER: Purpose<'static> = Purpose::Answer { model: "m" };

    // covers: AI-21
    // suite: unit
    #[test]
    fn a_closed_model_suggests_a_newer_one_where_known() {
        assert_eq!(newer_model("gemini"), crate::gateway::route::GEMINI_MODEL);
        assert_eq!(
            newer_model("xai"),
            "<model>",
            "no pick for other providers yet"
        );
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn http_statuses_map_to_plain_errors_that_name_the_provider() {
        use ProviderErrorKind as Kind;
        let refused = "Gemini refused the key. Store a new one with `priya ai key set gemini`.";
        let moved = "Gemini isn't at https://example.invalid/v1. Check its address in settings.";
        let cases = [
            (401, ANSWER, "", Kind::Auth, refused),
            (
                400,
                ANSWER,
                r#"[{"error":{"code":400,"status":"INVALID_ARGUMENT","details":[{"reason":"API_KEY_INVALID"}]}}]"#,
                Kind::Auth,
                refused,
            ),
            (
                400,
                ANSWER,
                "{}",
                Kind::BadRequest,
                "Gemini didn't accept the question.",
            ),
            (
                400,
                ANSWER,
                r#"[{"error":{"code":400,"message":"Thinking level MINIMAL is not supported for this model. Please retry with other thinking level.","status":"INVALID_ARGUMENT"}}]"#,
                Kind::BadRequest,
                "Gemini won't think at the level in settings for m. Change it with `priya settings set ai.providers.gemini.reasoning_effort <level>` (low, medium, high or default).",
            ),
            (
                404,
                ANSWER,
                r#"[{"error":{"code":404,"message":"This model models/m is no longer available to new users. Please update your code to use a newer model."}}]"#,
                Kind::BadRequest,
                "Gemini no longer offers m to new keys. Choose a newer model with `priya settings set ai.strategy fixed:gemini/gemini-3.8-flash`.",
            ),
            // A web page at a wrong address that says "no longer available"
            // is still a wrong address.
            (
                404,
                ANSWER,
                "<html>This page is no longer available</html>",
                Kind::BadRequest,
                moved,
            ),
            (
                403,
                ANSWER,
                "",
                Kind::Auth,
                "Gemini won't let this key use m.",
            ),
            (403, Purpose::Health, "", Kind::Auth, refused),
            (
                402,
                ANSWER,
                "",
                Kind::BadRequest,
                "Gemini says the account has no credit left.",
            ),
            (
                404,
                ANSWER,
                r#"{"error":{"message":"model 'm' not found"}}"#,
                Kind::BadRequest,
                "Gemini doesn't have the model m.",
            ),
            (
                404,
                ANSWER,
                "<html>Not Found</html>",
                Kind::BadRequest,
                moved,
            ),
            (404, Purpose::Health, "", Kind::BadRequest, moved),
            (301, ANSWER, "", Kind::BadRequest, moved),
            (
                408,
                ANSWER,
                "",
                Kind::Network,
                "Gemini didn't get the whole question in time.",
            ),
            (
                413,
                ANSWER,
                "",
                Kind::BadRequest,
                "The question is too long for Gemini.",
            ),
            (
                429,
                ANSWER,
                "",
                Kind::RateLimited,
                "Gemini is getting too many questions. Try again in a moment.",
            ),
            (
                503,
                ANSWER,
                "",
                Kind::Overloaded,
                "Gemini is busy right now.",
            ),
            (
                529,
                ANSWER,
                "",
                Kind::Overloaded,
                "Gemini is busy right now.",
            ),
            (
                500,
                ANSWER,
                "",
                Kind::Server,
                "Gemini had a problem on its side.",
            ),
            (
                418,
                ANSWER,
                "",
                Kind::BadRequest,
                "Gemini didn't accept the question.",
            ),
        ];
        for (status, purpose, body, kind, message) in cases {
            let error = status_error(&GEMINI, status, purpose, body, None);
            assert_eq!(
                (error.kind, error.user_message.as_str()),
                (kind, message),
                "{status}"
            );
        }
        let local = Provider {
            id: "local",
            label: "the local model",
            has_key: false,
            ..GEMINI
        };
        let needs = "The local model needs a key. Store one with `priya ai key set local`.";
        assert_eq!(
            status_error(&local, 401, ANSWER, "", None).user_message,
            needs
        );
        assert_eq!(
            status_error(&local, 403, ANSWER, "", None).user_message,
            needs
        );

        // A daily quota says how long, and keeps the wait.
        let hours = Some(Duration::from_secs(3 * 3600 - 10));
        let quota = status_error(&GEMINI, 429, ANSWER, "", hours);
        assert_eq!(
            quota.user_message,
            "Gemini's free limit is used up for now. Try again in about 3 hours."
        );
        assert_eq!(quota.retry_after, hours);
        let minutes = status_error(&GEMINI, 429, ANSWER, "", Some(Duration::from_secs(600)));
        assert!(minutes.user_message.ends_with("in about 10 minutes."));
        assert_eq!(plain_duration(Duration::from_secs(30)), "about a minute");
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn retry_after_reads_seconds_dates_and_geminis_retry_delay() {
        let now = time::macros::datetime!(2026-10-20 12:00:00 UTC);
        assert_eq!(
            retry_after(Some(" 7 "), "", now),
            Some(Duration::from_secs(7))
        );
        assert_eq!(
            retry_after(Some("Tue, 20 Oct 2026 12:00:30 GMT"), "", now),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            retry_after(Some("Tue, 20 Oct 2026 11:00:00 GMT"), "", now),
            Some(Duration::ZERO),
            "a date in the past means now"
        );
        let gemini = r#"{"error":{"details":[{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay": "33s"}]}}"#;
        assert_eq!(
            retry_after(None, gemini, now),
            Some(Duration::from_secs(33))
        );
        assert_eq!(retry_after(Some("soon"), "{}", now), None);
        let fraction = r#"{"retryDelay":
            "1.5s"}"#;
        assert_eq!(
            retry_after(None, fraction, now),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            retry_after(None, r#""retryDelay": "0.2s""#, now),
            Some(Duration::from_secs(1)),
            "rounded up, never zero"
        );
        assert_eq!(
            retry_after(Some("99999999999"), "", now),
            Some(LONGEST_WAIT),
            "at most a day"
        );
        assert_eq!(retry_after(None, "", now), None);
    }

    // covers: AI-21
    // suite: security
    #[test]
    fn details_never_carry_the_key_and_keep_their_causes() {
        assert_eq!(
            redact("bad key sk-123 for sk-123", Some("sk-123")),
            "bad key [key] for [key]"
        );
        assert_eq!(redact("text", None), "text");
        assert_eq!(redact("text", Some("")), "text");
        // Cut after removing the key, so no prefix of it is left.
        let key = "k".repeat(40);
        let body = format!("{}{key}", "x".repeat(280));
        let detail = body_detail("400 Bad Request", &body, Some(&key));
        assert!(detail.starts_with("HTTP 400 Bad Request: xxx"), "{detail}");
        assert!(!detail.contains("kkk"), "{detail}");

        #[derive(Debug)]
        struct Sending(std::io::Error);
        impl std::fmt::Display for Sending {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("error sending request")
            }
        }
        impl Error for Sending {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(&self.0)
            }
        }
        let refused =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused");
        assert_eq!(
            chain(&Sending(refused)),
            "error sending request: connection refused"
        );
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn error_bodies_are_cut_to_one_short_line() {
        assert_eq!(excerpt("{\n  \"error\": 1\n}"), "{ \"error\": 1 }");
        let long = "é".repeat(DETAIL_LIMIT + 10);
        let cut = excerpt(&long);
        assert_eq!(cut.chars().count(), DETAIL_LIMIT + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(capital(""), "");
        assert_eq!(capital("the local model"), "The local model");
    }
}
