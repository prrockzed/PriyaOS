//! `ai.strategy`: which model answers (04-priya-ai §4, ADR-015).
//!
//! Phase 0 has a single model, so only `fixed:<provider>/<model>` is
//! accepted. The other strategies (`best`, `balanced`, `cheapest`, and the
//! `family:<name>` and `local_only` filters, alone or combined with `,`, `+`
//! or spaces) are recognised and refused with a note that they arrive in
//! Phase 1, so a settings file written for later phases gets a clear answer
//! rather than "unknown value".
//!
//! Messages leave out the value: the settings loader adds it, with the file
//! and line, so it isn't said twice. `fixed` with filters
//! (`fixed:a/b,local_only`) is refused too: in Phase 1 it means a fixed model
//! plus a filter.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use priya_api::error::{ErrorKind, UserFacing};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use super::names::ProviderName;

/// The strategy bases and filters that come with routing in Phase 1.
const PHASE_ONE: &[&str] = &["best", "balanced", "cheapest", "local_only"];
const FAMILY: &str = "family:";

/// How Priya AI picks a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Strategy {
    /// Always this model from this provider.
    Fixed {
        provider: ProviderName,
        model: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StrategyError {
    #[error(
        "that strategy arrives in Phase 1; for now, pick one model with fixed:<provider>/<model>"
    )]
    NotYet(String),
    #[error(
        "a fixed strategy is written fixed:<provider>/<model>, for example fixed:anthropic/claude-sonnet-5-5"
    )]
    BadFixed(String),
    #[error("that isn't a strategy; use fixed:<provider>/<model>")]
    Unknown(String),
}

impl UserFacing for StrategyError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        // Said on its own, so it starts with a capital letter.
        let message = self.to_string();
        let mut chars = message.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    }
}

impl FromStr for Strategy {
    type Err = StrategyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (first, filters) = text
            .split_once(is_separator)
            .map_or((text, None), |(first, rest)| (first, Some(rest)));
        if let Some(rest) = first.strip_prefix("fixed:") {
            // `fixed` plus filters is a Phase 1 strategy; accepting the text
            // now as a model id would change its meaning later.
            if let Some(filters) = filters {
                return Err(if is_phase_one(filters) {
                    StrategyError::NotYet(text.to_owned())
                } else {
                    StrategyError::BadFixed(text.to_owned())
                });
            }
            // The model id may contain slashes itself (`meta-llama/Llama-3`),
            // so only the first one ends the provider.
            let bad = || StrategyError::BadFixed(text.to_owned());
            let (provider, model) = rest.split_once('/').ok_or_else(bad)?;
            if model.is_empty() {
                return Err(bad());
            }
            let provider = ProviderName::try_from(provider.to_owned()).map_err(|_| bad())?;
            return Ok(Self::Fixed {
                provider,
                model: model.to_owned(),
            });
        }
        if is_phase_one(text) {
            return Err(StrategyError::NotYet(text.to_owned()));
        }
        Err(StrategyError::Unknown(text.to_owned()))
    }
}

/// What joins a strategy base and its filters (Phase 1).
fn is_separator(c: char) -> bool {
    c == ',' || c == '+' || c.is_whitespace()
}

/// Every part is a Phase 1 base or filter, as a whole word.
fn is_phase_one(text: &str) -> bool {
    let mut parts = text
        .split(is_separator)
        .filter(|part| !part.is_empty())
        .peekable();
    parts.peek().is_some()
        && parts.all(|part| {
            PHASE_ONE.contains(&part)
                || part
                    .strip_prefix(FAMILY)
                    .is_some_and(|family| !family.is_empty())
        })
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed { provider, model } => write!(f, "fixed:{provider}/{model}"),
        }
    }
}

impl TryFrom<String> for Strategy {
    type Error = StrategyError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<Strategy> for String {
    fn from(strategy: Strategy) -> Self {
        strategy.to_string()
    }
}

impl JsonSchema for Strategy {
    fn schema_name() -> Cow<'static, str> {
        "Strategy".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "Which model answers, written fixed:<provider>/<model>. Strategies that choose a model for you come later.",
            "pattern": format!(
                "^fixed:{}/[^\\s,+]+$",
                ProviderName::PATTERN.trim_start_matches('^').trim_end_matches('$')
            ),
            "examples": ["fixed:anthropic/claude-sonnet-5-5", "fixed:ollama/llama3.2:3b"],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-03
    // suite: unit
    // acceptance: 0.6
    #[test]
    fn only_fixed_strategy_accepted_in_phase0() {
        assert_eq!(
            "fixed:anthropic/claude-sonnet-5-5".parse(),
            Ok(Strategy::Fixed {
                provider: ProviderName::try_from("anthropic".to_owned()).unwrap(),
                model: "claude-sonnet-5-5".into()
            })
        );
        // Model ids may have slashes and colons of their own.
        let local: Strategy = "fixed:lm-studio/meta-llama/Llama-3:8b".parse().unwrap();
        assert_eq!(local.to_string(), "fixed:lm-studio/meta-llama/Llama-3:8b");

        for later in [
            "best",
            "balanced",
            "cheapest",
            "local_only",
            "family:claude",
            "cheapest+family:claude",
            "best, local_only",
            "best\tlocal_only",
            // Fixed plus filters means something else in Phase 1.
            "fixed:a/b,local_only",
            "fixed:a/b+family:x",
        ] {
            let err = later.parse::<Strategy>().unwrap_err();
            assert_eq!(err, StrategyError::NotYet(later.into()));
            assert!(err.user_message().contains("arrives in Phase 1"), "{err}");
            assert!(err.user_message().starts_with("That strategy "), "{err}");
        }
        for bad in [
            "fixed:",
            "fixed:claude",
            "fixed:/claude",
            "fixed:anthropic/",
            "fixed:Anthropic/claude",
            "fixed:anthropic/claude sonnet",
            "fixed:anthropic/claude\n",
            "fixed:a/b,c",
        ] {
            assert_eq!(
                bad.parse::<Strategy>(),
                Err(StrategyError::BadFixed(bad.into())),
                "{bad}"
            );
        }
        for unknown in [
            "fastest",
            "bestest",
            "cheapest2",
            "family:",
            "",
            " fixed:a/b",
        ] {
            assert_eq!(
                unknown.parse::<Strategy>(),
                Err(StrategyError::Unknown(unknown.into())),
                "{unknown:?}"
            );
        }
        let unknown = StrategyError::Unknown("x".into()).user_message();
        assert_eq!(
            unknown,
            "That isn't a strategy; use fixed:<provider>/<model>"
        );

        // Through serde, as a settings file is read.
        let parsed: Strategy = serde_json::from_str("\"fixed:ollama/llama3.2:3b\"").unwrap();
        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            "\"fixed:ollama/llama3.2:3b\""
        );
        let err = serde_json::from_str::<Strategy>("\"cheapest\"").unwrap_err();
        assert!(err.to_string().contains("arrives in Phase 1"), "{err}");
    }
}
