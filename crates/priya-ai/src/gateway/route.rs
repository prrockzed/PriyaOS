//! Which provider and model answer: `ai.strategy` (`fixed:<provider>/<model>`
//! in Phase 0) or a task's `--model <provider>/<model>` override, checked
//! against `ai.providers`. Routing proper comes with 1.9.

use priya_api::error::{ErrorKind, UserFacing};
use priya_core::settings::{Ai, ProviderKind, ProviderName, ReasoningEffort, Strategy};

/// A model chosen to answer, and where to reach it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub provider: ProviderName,
    pub model: String,
    pub base_url: String,
    /// The server runs on this computer, so nothing leaves it: the answer
    /// says "on this laptop". A key is still sent if one is stored (a local
    /// proxy may want one).
    pub local: bool,
    /// How messages name the provider mid-sentence.
    pub label: String,
    /// The provider's `reasoning_effort` setting, sent with each request.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Why no model can answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RouteError {
    #[error("ai.strategy is not set")]
    NoModel,
    #[error("`{0}` isn't written <provider>/<model>")]
    BadOverride(String),
    #[error("no provider named `{0}` in ai.providers")]
    UnknownProvider(String),
    #[error("provider `{0}` is turned off")]
    Disabled(String),
    #[error("provider `{0}` has no base_url")]
    NoAddress(String),
    #[error("provider `{0}` is of a kind that arrives in Phase 1")]
    NotYet(String),
}

impl UserFacing for RouteError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::NoModel => ErrorKind::NotFound,
            Self::BadOverride(_) => ErrorKind::InvalidInput,
            _ => ErrorKind::Unavailable,
        }
    }

    fn retryable(&self) -> bool {
        false
    }

    fn user_message(&self) -> String {
        match self {
            // The words of the approved bar mockup (0.13).
            Self::NoModel => format!(
                "Priya needs a model to answer. Run one of these in a terminal, then ask again. A free cloud model (Gemini): `priya ai key set gemini`, then `priya settings set ai.strategy fixed:gemini/{GEMINI_MODEL}`. Or a model on this laptop (Ollama): `priya settings set ai.strategy fixed:local/{LOCAL_MODEL}`."
            ),
            Self::BadOverride(text) => {
                format!(
                    "Name the model as provider/model, such as local/{LOCAL_MODEL}, not `{text}`."
                )
            }
            Self::UnknownProvider(name) => format!(
                "There's no provider called {name}. `priya settings get ai.providers` lists them."
            ),
            Self::Disabled(name) => format!(
                "The provider {name} is turned off. Turn it on with `priya settings set ai.providers.{name}.enabled true`."
            ),
            Self::NoAddress(name) => format!(
                "The provider {name} has no address. Set one with `priya settings set ai.providers.{name}.base_url <address>`."
            ),
            Self::NotYet(name) => format!(
                "The provider {name} speaks an API that comes later. For now, use one that speaks the OpenAI API."
            ),
        }
    }
}

/// The models the "no model yet" hint suggests: Gemini's free tier, and a
/// small model Ollama pulls quickly.
// Gemini 2.5 is closed to new API keys (2026); 3.8 Flash is Google's pick
// for new projects and has a free tier.
pub const GEMINI_MODEL: &str = "gemini-3.8-flash";
pub const LOCAL_MODEL: &str = "qwen2.5:0.5b";

/// The route for a question: `model` (`<provider>/<model>`) when given,
/// else `ai.strategy`.
pub fn route(ai: &Ai, model: Option<&str>) -> Result<Route, RouteError> {
    let (provider, model) = match model {
        Some(text) => {
            let bad = || RouteError::BadOverride(text.to_owned());
            let (provider, model) = text.split_once('/').ok_or_else(bad)?;
            if model.is_empty() {
                return Err(bad());
            }
            let provider = ProviderName::try_from(provider.to_owned()).map_err(|_| bad())?;
            (provider, model.to_owned())
        }
        None => match &ai.strategy {
            Some(Strategy::Fixed { provider, model }) => (provider.clone(), model.clone()),
            None => return Err(RouteError::NoModel),
        },
    };
    let name = provider.as_str().to_owned();
    let settings = ai
        .providers
        .get(&provider)
        .ok_or_else(|| RouteError::UnknownProvider(name.clone()))?;
    if !settings.enabled {
        return Err(RouteError::Disabled(name));
    }
    if settings.kind != ProviderKind::OpenaiCompatible {
        return Err(RouteError::NotYet(name));
    }
    let base_url = settings
        .base_url
        .clone()
        .ok_or_else(|| RouteError::NoAddress(name.clone()))?;
    Ok(Route {
        local: is_local(&base_url),
        label: label(&name),
        provider,
        model,
        base_url,
        reasoning_effort: settings.reasoning_effort,
    })
}

/// Whether a server address is on this computer: a loopback address or
/// `localhost`. Parsed as a URL, so `http://127.0.0.1@example.com` (a user
/// name, then a remote host) or `127.example.com` aren't mistaken for it;
/// saying "on this laptop" about a cloud server would mislead.
pub fn is_local(base_url: &str) -> bool {
    use url::Host;
    let Ok(url) = url::Url::parse(base_url) else {
        return false;
    };
    match url.host() {
        Some(Host::Domain(name)) => name.trim_end_matches('.').eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => {
            ip.is_loopback() || ip.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
        None => false,
    }
}

/// How messages name a provider mid-sentence: its product name when known,
/// else the name from settings.
pub fn label(provider: &str) -> String {
    match provider {
        "gemini" => "Gemini".into(),
        "xai" | "grok" => "Grok".into(),
        "openai" => "OpenAI".into(),
        "local" => "the local model server".into(),
        "ollama" => "Ollama".into(),
        "lm-studio" | "lmstudio" => "LM Studio".into(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use priya_core::settings::Provider;

    use super::*;

    fn ai(strategy: Option<&str>) -> Ai {
        let provider = |url: &str, enabled| Provider {
            kind: ProviderKind::OpenaiCompatible,
            base_url: Some(url.to_owned()),
            enabled,
            reasoning_effort: None,
        };
        let name = |n: &str| ProviderName::try_from(n.to_owned()).unwrap();
        let mut providers = BTreeMap::new();
        providers.insert(name("local"), provider("http://127.0.0.1:11434/v1", true));
        providers.insert(
            name("gemini"),
            Provider {
                reasoning_effort: Some(ReasoningEffort::Minimal),
                ..provider("https://example.invalid/openai", true)
            },
        );
        providers.insert(name("off"), provider("http://localhost:1234/v1", false));
        providers.insert(
            name("claude"),
            Provider {
                kind: ProviderKind::Anthropic,
                base_url: None,
                enabled: true,
                reasoning_effort: None,
            },
        );
        providers.insert(
            name("nowhere"),
            Provider {
                kind: ProviderKind::OpenaiCompatible,
                base_url: None,
                enabled: true,
                reasoning_effort: None,
            },
        );
        Ai {
            strategy: strategy.map(|s| s.parse().unwrap()),
            providers,
            ..Ai::default()
        }
    }

    // covers: AI-21
    // suite: unit
    // acceptance: 0.12
    #[test]
    fn strategy_fixed_resolves_provider_and_model() {
        let route = route(&ai(Some("fixed:gemini/gemini-3.8-flash")), None).unwrap();
        assert_eq!(route.provider.as_str(), "gemini");
        assert_eq!(route.model, "gemini-3.8-flash");
        assert_eq!(route.base_url, "https://example.invalid/openai");
        assert!(!route.local);
        assert_eq!(route.label, "Gemini");
        assert_eq!(
            route.reasoning_effort,
            Some(ReasoningEffort::Minimal),
            "the provider's setting comes along"
        );

        // A task's override wins, and model names keep their slashes.
        let local = super::route(&ai(Some("fixed:gemini/x")), Some("local/meta/llama:3b")).unwrap();
        assert_eq!(
            (local.provider.as_str(), local.model.as_str()),
            ("local", "meta/llama:3b")
        );
        assert!(local.local);
        assert_eq!(local.label, "the local model server");
        assert_eq!(local.reasoning_effort, None);

        for (strategy, error) in [
            (
                "fixed:missing/m",
                RouteError::UnknownProvider("missing".into()),
            ),
            ("fixed:off/m", RouteError::Disabled("off".into())),
            ("fixed:claude/m", RouteError::NotYet("claude".into())),
            ("fixed:nowhere/m", RouteError::NoAddress("nowhere".into())),
        ] {
            assert_eq!(
                super::route(&ai(Some(strategy)), None),
                Err(error),
                "{strategy}"
            );
        }
        for bad in ["local", "local/", "Local/m", "/m"] {
            assert_eq!(
                super::route(&ai(None), Some(bad)),
                Err(RouteError::BadOverride(bad.into()))
            );
        }
    }

    // covers: AI-21
    // suite: unit
    // acceptance: 0.12
    #[test]
    fn no_strategy_gives_connect_hint() {
        let error = route(&ai(None), None).unwrap_err();
        assert_eq!(error, RouteError::NoModel);
        let message = error.user_message();
        assert!(message.contains("priya ai key set gemini"), "{message}");
        assert!(
            message.contains("fixed:gemini/gemini-3.8-flash"),
            "{message}"
        );
        assert!(message.contains("fixed:local/qwen2.5:0.5b"), "{message}");
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert!(!error.retryable());
        for other in [
            RouteError::BadOverride("x".into()),
            RouteError::UnknownProvider("x".into()),
            RouteError::Disabled("x".into()),
            RouteError::NoAddress("x".into()),
            RouteError::NotYet("x".into()),
        ] {
            let message = other.user_message();
            assert!(
                message.ends_with('.') && !message.contains("ai.strategy is"),
                "{message}"
            );
        }
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn local_addresses_and_labels() {
        for local in [
            "http://127.0.0.1:11434/v1",
            "http://localhost:1234/v1",
            "http://LOCALHOST./v1",
            "http://[::1]:8080/v1",
            "http://[::ffff:127.0.0.1]:8080/v1",
            "http://127.0.0.2/v1",
        ] {
            assert!(is_local(local), "{local}");
        }
        for remote in [
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "http://192.168.1.20:1234/v1",
            "http://localhost.example.com/v1",
            "http://[2001:db8::1]:80/v1",
            "http://127.0.0.1@example.com/v1",
            "http://127.0.0.1:80@example.com/v1",
            "http://127.0.0.1.example.com/v1",
            "http://127.example.com/v1",
            "http://0.0.0.0:11434/v1",
            "not a url",
        ] {
            assert!(!is_local(remote), "{remote}");
        }
        assert_eq!(label("xai"), "Grok");
        assert_eq!(label("lm-studio"), "LM Studio");
        assert_eq!(label("work-server"), "work-server");
    }
}
