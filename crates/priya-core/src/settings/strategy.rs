//! `ai.strategy`: which model answers (04-priya-ai §4, ADR-015).
//!
//! Phase 0 has a single model, so only `fixed:<provider>/<model>` is
//! accepted. The other strategies (`best`, `balanced`, `cheapest`, and the
//! `family:` and `local_only` filters) are recognised and refused with a
//! note that they arrive in Phase 1, so a settings file written for later
//! phases gets a clear answer rather than "unknown value".

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use priya_api::error::{ErrorKind, UserFacing};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

/// The strategies and filters that come with routing in Phase 1.
const PHASE_ONE: &[&str] = &["best", "balanced", "cheapest", "local_only", "family:"];

/// How Priya AI picks a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Strategy {
    /// Always this model from this provider.
    Fixed { provider: String, model: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StrategyError {
    #[error(
        "the {0:?} strategy arrives in Phase 1; for now, pick one model with fixed:<provider>/<model>"
    )]
    NotYet(String),
    #[error(
        "write a fixed strategy as fixed:<provider>/<model>, for example fixed:anthropic/claude-sonnet-5-5; you wrote {0:?}"
    )]
    BadFixed(String),
    #[error("{0:?} isn't a strategy; use fixed:<provider>/<model>")]
    Unknown(String),
}

impl UserFacing for StrategyError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::InvalidInput
    }

    fn user_message(&self) -> String {
        // Already plain; settings errors add the file and line around it.
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
        let text = text.trim();
        if let Some(rest) = text.strip_prefix("fixed:") {
            // The model id may contain slashes itself (`meta-llama/Llama-3`),
            // so only the first one ends the provider.
            return match rest.split_once('/') {
                Some((provider, model)) if is_provider_name(provider) && is_model_id(model) => {
                    Ok(Self::Fixed {
                        provider: provider.to_owned(),
                        model: model.to_owned(),
                    })
                }
                _ => Err(StrategyError::BadFixed(text.to_owned())),
            };
        }
        if PHASE_ONE.iter().any(|later| text.starts_with(later)) {
            return Err(StrategyError::NotYet(text.to_owned()));
        }
        Err(StrategyError::Unknown(text.to_owned()))
    }
}

/// The names allowed for `ai.providers.<name>`.
fn is_provider_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn is_model_id(model: &str) -> bool {
    !model.is_empty() && !model.chars().any(char::is_whitespace)
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
            "description": "Which model answers. Phase 0 accepts only fixed:<provider>/<model>.",
            "pattern": "^fixed:[a-z0-9][a-z0-9_-]*/\\S+$",
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
                provider: "anthropic".into(),
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
        ] {
            let err = later.parse::<Strategy>().unwrap_err();
            assert_eq!(err, StrategyError::NotYet(later.into()));
            assert!(err.user_message().contains("arrives in Phase 1"), "{err}");
            assert!(err.user_message().starts_with("The "), "{err}");
        }
        for bad in [
            "fixed:",
            "fixed:claude",
            "fixed:/claude",
            "fixed:anthropic/",
            "fixed:Anthropic/claude",
            "fixed:anthropic/claude sonnet",
        ] {
            assert_eq!(
                bad.parse::<Strategy>(),
                Err(StrategyError::BadFixed(bad.into())),
                "{bad}"
            );
        }
        assert!(matches!(
            "fastest".parse::<Strategy>(),
            Err(StrategyError::Unknown(_))
        ));

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
