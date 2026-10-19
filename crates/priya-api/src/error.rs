//! The one error model that every PriyaOS crate maps its errors into before
//! they cross the API boundary.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What kind of failure happened. Clients branch on this, never on messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The request was malformed or a value was out of range.
    InvalidInput,
    NotFound,
    /// The request conflicts with the current state, such as a stale revision.
    Conflict,
    PermissionDenied,
    /// Something needed (the daemon, a provider, the network) can't be reached.
    Unavailable,
    Timeout,
    Cancelled,
    /// A model provider returned an error.
    ProviderError,
    /// A bug in PriyaOS.
    Internal,
}

impl ErrorKind {
    /// Whether a failure of this kind usually goes away if the request is sent
    /// again later. Errors can override this case by case.
    pub const fn retryable_by_default(self) -> bool {
        matches!(self, Self::Unavailable | Self::Timeout)
    }

    /// The stable wire name, as in the JSON form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::PermissionDenied => "permission_denied",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::ProviderError => "provider_error",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The error every API method returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApiError {
    pub kind: ErrorKind,
    /// True when the same request may succeed if sent again later.
    pub retryable: bool,
    /// One plain sentence that can be shown to the user as is.
    pub user_message: String,
    /// Technical detail for logs and the "Details" view; never shown by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ApiError {
    pub fn new(kind: ErrorKind, user_message: impl Into<String>) -> Self {
        Self {
            kind,
            retryable: kind.retryable_by_default(),
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
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    /// Converts a crate error into the wire form. Its `Display` text becomes
    /// the detail, so the user message stays plain.
    pub fn from_user_facing<E: UserFacing + ?Sized>(err: &E) -> Self {
        Self {
            kind: err.kind(),
            retryable: err.retryable(),
            user_message: err.user_message(),
            detail: Some(err.to_string()),
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.user_message)
    }
}

impl std::error::Error for ApiError {}

/// Implemented by every crate's error enum, so any error can become an
/// [`ApiError`] without losing its kind or its plain-language message.
pub trait UserFacing: std::error::Error {
    fn kind(&self) -> ErrorKind;

    fn retryable(&self) -> bool {
        self.kind().retryable_by_default()
    }

    /// One plain sentence for the user, without jargon or internal names.
    fn user_message(&self) -> String;
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // covers: CORE-04
    // suite: unit
    // acceptance: 0.2
    // regression: 0.2
    #[test]
    fn api_error_serializes_kind_retryable_user_message() {
        let err = ApiError::new(
            ErrorKind::PermissionDenied,
            "Priya isn't allowed to do that.",
        );
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            json!({
                "kind": "permission_denied",
                "retryable": false,
                "user_message": "Priya isn't allowed to do that.",
            })
        );

        let err = ApiError::new(ErrorKind::Timeout, "The model took too long.").with_detail("30 s");
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["kind"], "timeout");
        assert_eq!(value["retryable"], true);
        assert_eq!(value["detail"], "30 s");
        assert_eq!(serde_json::from_value::<ApiError>(value).unwrap(), err);
    }

    // covers: CORE-04
    // suite: unit
    // acceptance: 0.2
    // regression: 0.2
    #[test]
    fn every_kind_has_a_stable_wire_name() {
        let all = [
            ErrorKind::InvalidInput,
            ErrorKind::NotFound,
            ErrorKind::Conflict,
            ErrorKind::PermissionDenied,
            ErrorKind::Unavailable,
            ErrorKind::Timeout,
            ErrorKind::Cancelled,
            ErrorKind::ProviderError,
            ErrorKind::Internal,
        ];
        for kind in all {
            assert_eq!(serde_json::to_value(kind).unwrap(), kind.as_str());
        }
    }

    #[derive(Debug)]
    struct DiskFull;

    impl fmt::Display for DiskFull {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("ENOSPC writing /var/lib/priya/blobs")
        }
    }

    impl std::error::Error for DiskFull {}

    impl UserFacing for DiskFull {
        fn kind(&self) -> ErrorKind {
            ErrorKind::Unavailable
        }

        fn retryable(&self) -> bool {
            false
        }

        fn user_message(&self) -> String {
            "The disk is full.".into()
        }
    }

    // covers: CORE-04
    // suite: unit
    // acceptance: 0.2
    // regression: 0.2
    #[test]
    fn user_facing_error_keeps_kind_and_plain_message() {
        let api = ApiError::from_user_facing(&DiskFull);
        assert_eq!(api.kind, ErrorKind::Unavailable);
        assert!(
            !api.retryable,
            "the error's own retryable wins over the default"
        );
        assert_eq!(api.user_message, "The disk is full.");
        assert_eq!(
            api.detail.as_deref(),
            Some("ENOSPC writing /var/lib/priya/blobs")
        );
    }
}
