//! [`ProviderError`]: how a model call failed, in a form both the router
//! (kind, retryable, retry after) and the user (one plain sentence) can use.

use std::time::Duration;

use priya_api::error::{ErrorKind, UserFacing};

/// What went wrong with a model call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProviderErrorKind {
    /// The key is missing, wrong or not allowed to use this model.
    Auth,
    /// Too many requests; free tiers hit this often.
    RateLimited,
    /// The provider is busy and asked to come back later.
    Overloaded,
    /// The provider refused the request itself (unknown model, too long).
    BadRequest,
    /// It couldn't be reached, or the connection broke.
    Network,
    /// It took longer than the timeouts allow.
    Timeout,
    /// The provider failed on its side.
    Server,
}

impl ProviderErrorKind {
    /// Whether asking again later may work.
    pub const fn retryable(self) -> bool {
        !matches!(self, Self::Auth | Self::BadRequest)
    }

    /// The stable name, used in events and logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::RateLimited => "rate_limited",
            Self::Overloaded => "overloaded",
            Self::BadRequest => "bad_request",
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::Server => "server",
        }
    }
}

/// A failed model call. `detail` is for logs and "Details" and never holds
/// key material: adapters build it from status codes and response bodies,
/// never from the request headers. `Display` includes the detail, because
/// that text becomes the API error's detail.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind}: {user_message}{}", detail.as_ref().map(|d| format!(" ({d})")).unwrap_or_default(), kind = kind.as_str())]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub retryable: bool,
    /// How long the provider asked us to wait (`Retry-After`).
    pub retry_after: Option<Duration>,
    pub user_message: String,
    pub detail: Option<String>,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, user_message: impl Into<String>) -> Self {
        Self {
            kind,
            retryable: kind.retryable(),
            retry_after: None,
            user_message: user_message.into(),
            detail: None,
        }
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[must_use]
    pub fn with_retry_after(mut self, wait: Option<Duration>) -> Self {
        self.retry_after = wait;
        self
    }
}

impl UserFacing for ProviderError {
    fn kind(&self) -> ErrorKind {
        match self.kind {
            ProviderErrorKind::Network => ErrorKind::Unavailable,
            ProviderErrorKind::Timeout => ErrorKind::Timeout,
            _ => ErrorKind::ProviderError,
        }
    }

    fn retryable(&self) -> bool {
        self.retryable
    }

    fn user_message(&self) -> String {
        self.user_message.clone()
    }
}

#[cfg(test)]
mod tests {
    use priya_api::error::ApiError;

    use super::*;

    // covers: AI-21
    // suite: unit
    #[test]
    fn provider_errors_keep_kind_and_plain_message_across_the_api() {
        let busy = ProviderError::new(ProviderErrorKind::Overloaded, "Gemini is busy right now.")
            .with_retry_after(Some(Duration::from_secs(3)))
            .with_detail("HTTP 503");
        assert!(busy.retryable);
        let api = ApiError::from_user_facing(&busy);
        assert_eq!(api.kind, ErrorKind::ProviderError);
        assert!(api.retryable);
        assert_eq!(api.user_message, "Gemini is busy right now.");
        assert_eq!(
            api.detail.as_deref(),
            Some("overloaded: Gemini is busy right now. (HTTP 503)")
        );

        let auth = ProviderError::new(ProviderErrorKind::Auth, "The Gemini key was refused.");
        assert!(!auth.retryable);
        assert_eq!(auth.to_string(), "auth: The Gemini key was refused.");
        assert!(!ApiError::from_user_facing(&auth).retryable);
        let offline = ProviderError::new(ProviderErrorKind::Network, "x");
        assert_eq!(UserFacing::kind(&offline), ErrorKind::Unavailable);
        let slow = ProviderError::new(ProviderErrorKind::Timeout, "x");
        assert_eq!(UserFacing::kind(&slow), ErrorKind::Timeout);
        assert!(!ProviderErrorKind::BadRequest.retryable());
        assert_eq!(ProviderErrorKind::RateLimited.as_str(), "rate_limited");
    }
}
