//! How Priya AI is set up: the model chosen, its key, whether its provider
//! answers. Shared by `priya ai status` and the `ai.*` doctor checks.

use priya_ai::gateway::{ConnectError, Route, RouteError, route};
use priya_ai::keys::KeyError;
use priya_ai::{Health, ProviderError};
use priya_api::error::UserFacing;
use priya_api::methods::ai::{AiStatus, KeyState};
use priya_core::settings::Ai;

use super::answer::Answerer;

/// The longest status and doctor wait on the keyring. A locked keyring may
/// ask for a password; they report that rather than wait for it.
pub const KEYRING_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// `work` on the keyring, or `Locked` once it takes longer than
/// [`KEYRING_WAIT`].
pub async fn keyring<T>(work: impl Future<Output = Result<T, KeyError>>) -> Result<T, KeyError> {
    tokio::time::timeout(KEYRING_WAIT, work)
        .await
        .unwrap_or(Err(KeyError::Locked))
}

/// What is known about the model chosen.
pub struct Setup {
    pub route: Result<Route, RouteError>,
    /// Whether its key is stored; `None` without a route.
    pub key: Option<Result<KeyState, KeyError>>,
}

impl Setup {
    pub async fn read(answerer: &Answerer, ai: &Ai) -> Self {
        let route = route(ai, None);
        let key = match &route {
            Ok(route) => Some(key_state(answerer, route).await),
            Err(_) => None,
        };
        Self { route, key }
    }

    /// Asks the provider whether it answers, without spending tokens.
    pub async fn health(&self, answerer: &Answerer) -> Option<Health> {
        let route = self.route.as_ref().ok()?;
        Some(health(answerer, route).await)
    }
}

/// Whether `route`'s provider answers, without spending tokens.
pub async fn health(answerer: &Answerer, route: &Route) -> Health {
    let provider = tokio::time::timeout(KEYRING_WAIT, answerer.providers().provider(route))
        .await
        .unwrap_or(Err(ConnectError::Keyring(KeyError::Locked)));
    match provider {
        Ok(provider) => provider.health().await,
        // No key, or no keyring: it can't be asked.
        Err(e) => Health::Down(ProviderError::new(
            priya_ai::ProviderErrorKind::Auth,
            e.user_message(),
        )),
    }
}

/// Whether `route`'s key is stored (one keyring read).
pub async fn key_state(answerer: &Answerer, route: &Route) -> Result<KeyState, KeyError> {
    let stored = keyring(answerer.providers().key_stored(route.provider.as_str())).await;
    match (stored, route.local) {
        (Ok(true), _) => Ok(KeyState::Stored),
        (Ok(false) | Err(_), true) => Ok(KeyState::NotNeeded),
        (Ok(false), false) => Ok(KeyState::Missing),
        (Err(e), false) => Err(e),
    }
}

/// `v1.ai.status`: the setup, and with `check`, whether the provider
/// answers (the keyring and the provider get 2 s each).
pub async fn status(answerer: &Answerer, ai: &Ai, check: bool) -> AiStatus {
    let setup = Setup::read(answerer, ai).await;
    let health = if check {
        setup.health(answerer).await
    } else {
        None
    };
    let (route, problem) = match &setup.route {
        Ok(route) => (Some(route), None),
        Err(e) => (None, Some(e.user_message())),
    };
    let problem = problem.or_else(|| match &setup.key {
        Some(Err(e)) => Some(e.user_message()),
        _ => None,
    });
    let problem = problem.or_else(|| match &health {
        Some(Health::Down(e)) => Some(e.user_message.clone()),
        _ => None,
    });
    AiStatus {
        configured: route.is_some(),
        provider: route.map(|r| r.provider.as_str().to_owned()),
        model: route.map(|r| r.model.clone()),
        local: route.map(|r| r.local),
        key: setup.key.and_then(Result::ok),
        reachable: health.map(|h| h == Health::Ok),
        problem,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use priya_ai::ModelProvider;
    use priya_ai::provider::BoxFuture;
    use priya_core::clock::SystemClock;
    use priya_core::settings::{Provider, ProviderKind, ProviderName};
    use priya_store::{EventStore, StoreConfig};
    use time::UtcOffset;

    use super::super::answer::ProviderSource;
    use super::*;

    /// A keyring waiting forever on its unlock prompt.
    struct Stuck;

    impl ProviderSource for Stuck {
        fn provider<'a>(
            &'a self,
            _: &'a Route,
        ) -> BoxFuture<'a, Result<Arc<dyn ModelProvider>, ConnectError>> {
            Box::pin(std::future::pending())
        }

        fn key_stored<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<bool, KeyError>> {
            Box::pin(std::future::pending())
        }
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn a_keyring_that_doesnt_answer_is_reported_not_waited_for() {
        let dir = tempfile::tempdir().unwrap();
        let store =
            EventStore::open(&StoreConfig::in_data_dir(dir.path()), Arc::new(SystemClock)).unwrap();
        let answerer = Answerer::new(
            Arc::new(Stuck),
            store,
            Arc::new(SystemClock),
            String::new(),
            UtcOffset::UTC,
        );
        let mut providers = BTreeMap::new();
        providers.insert(
            ProviderName::try_from("gemini".to_owned()).unwrap(),
            Provider {
                kind: ProviderKind::OpenaiCompatible,
                base_url: Some("https://generativelanguage.googleapis.com/v1beta/openai".into()),
                enabled: true,
            },
        );
        let ai = Ai {
            strategy: Some("fixed:gemini/gemini-2.5-flash".parse().unwrap()),
            providers,
            max_output_tokens: 64,
        };
        let begun = tokio::time::Instant::now();
        let status = status(&answerer, &ai, true).await;
        // The keyring (2 s) for the key, then again for the provider.
        assert!(begun.elapsed() < 3 * KEYRING_WAIT);
        assert_eq!(status.key, None, "not known");
        assert_eq!(status.reachable, Some(false));
        assert!(status.problem.unwrap().contains("locked"));
    }
}
