//! The provider for a route: its key read from the keyring now (keys are
//! never kept between questions), the `openai_compatible` adapter, and
//! `Reliable` around it.

use std::sync::Arc;

use priya_api::error::{ErrorKind, UserFacing};
use reqwest::Client;

use super::Route;
use crate::error::ProviderError;
use crate::keys::{KeyError, SecretStore};
use crate::openai_compatible::errors::capital;
use crate::openai_compatible::{Endpoint, OpenAiCompatible, http_client};
use crate::provider::ModelProvider;
use crate::reliability::{Reliable, Timeouts};

/// Why a route's provider couldn't be set up.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectError {
    #[error("{provider} has no key stored")]
    NeedsKey { provider: String, label: String },
    #[error(transparent)]
    Keyring(#[from] KeyError),
}

impl UserFacing for ConnectError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::NeedsKey { .. } => ErrorKind::PermissionDenied,
            Self::Keyring(e) => e.kind(),
        }
    }

    fn retryable(&self) -> bool {
        false
    }

    fn user_message(&self) -> String {
        match self {
            Self::NeedsKey { provider, label } => format!(
                "{} needs a key. Store one with `priya ai key set {provider}`.",
                capital(label)
            ),
            Self::Keyring(e) => e.user_message(),
        }
    }
}

/// Builds providers for routes; one per daemon, sharing a connection pool.
pub struct Connector {
    keys: Arc<dyn SecretStore>,
    http: Client,
    timeouts: Timeouts,
}

impl Connector {
    pub fn new(keys: Arc<dyn SecretStore>, timeouts: Timeouts) -> Result<Self, ProviderError> {
        Ok(Self {
            keys,
            http: http_client()?,
            timeouts,
        })
    }

    /// Whether a key is stored for `provider`; reading the keyring also
    /// shows whether there is one.
    pub async fn key_stored(&self, provider: &str) -> Result<bool, KeyError> {
        Ok(self.keys.get(provider).await?.is_some())
    }

    /// The provider for `route`, ready to ask.
    pub async fn provider(&self, route: &Route) -> Result<Arc<dyn ModelProvider>, ConnectError> {
        let key = self.key(route).await?;
        let endpoint = Endpoint {
            id: route.provider.as_str().to_owned(),
            label: route.label.clone(),
            base_url: route.base_url.clone(),
            key: key.map(Arc::new),
        };
        let adapter = OpenAiCompatible::new(endpoint, self.http.clone());
        Ok(Arc::new(Reliable::new(Arc::new(adapter), self.timeouts)))
    }

    /// A cloud provider needs its key, and a keyring that can't be read is
    /// an error; asking without one would only fail at the provider, after
    /// sending the question. A local server may have a key but needn't,
    /// so it works without a keyring.
    async fn key(
        &self,
        route: &Route,
    ) -> Result<Option<priya_core::secret::Secret<String>>, ConnectError> {
        let stored = self.keys.get(route.provider.as_str()).await;
        match (stored, route.local) {
            (Ok(Some(key)), _) => Ok(Some(key)),
            (Ok(None), true) => Ok(None),
            (Err(e), true) => {
                // Most local servers need no key; say why one wasn't sent,
                // in case this one does.
                tracing::warn!(provider = route.provider.as_str(), error = %e, "no keyring; asking the local server without a key");
                Ok(None)
            }
            (Ok(None), false) => Err(ConnectError::NeedsKey {
                provider: route.provider.as_str().to_owned(),
                label: route.label.clone(),
            }),
            (Err(e), false) => Err(e.into()),
        }
    }
}
