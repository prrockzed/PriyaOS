//! JSON-RPC 2.0 messages (02-core §4), and how [`ApiError`]s travel in them.
//!
//! Every error carries `data: {kind, retryable, user_message}` (an
//! [`ApiError`]), so a client can show the plain message and branch on the
//! kind without parsing text. The standard codes cover malformed traffic;
//! application errors use one code per [`ErrorKind`].

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::error::{ApiError, ErrorKind};

/// Error codes.
pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    pub const NOT_FOUND: i64 = -32001;
    pub const INVALID_INPUT: i64 = -32002;
    pub const CONFLICT: i64 = -32003;
    pub const PERMISSION_DENIED: i64 = -32004;
    pub const UNAVAILABLE: i64 = -32005;
    pub const TIMEOUT: i64 = -32006;
    pub const CANCELLED: i64 = -32007;
    pub const PROVIDER_ERROR: i64 = -32008;
}

/// The application error code for each kind.
pub const fn code_for(kind: ErrorKind) -> i64 {
    match kind {
        ErrorKind::NotFound => code::NOT_FOUND,
        ErrorKind::InvalidInput => code::INVALID_INPUT,
        ErrorKind::Conflict => code::CONFLICT,
        ErrorKind::PermissionDenied => code::PERMISSION_DENIED,
        ErrorKind::Unavailable => code::UNAVAILABLE,
        ErrorKind::Timeout => code::TIMEOUT,
        ErrorKind::Cancelled => code::CANCELLED,
        ErrorKind::ProviderError => code::PROVIDER_ERROR,
        ErrorKind::Internal => code::INTERNAL_ERROR,
    }
}

/// The `"jsonrpc": "2.0"` member; any other value is refused.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct V2;

impl Serialize for V2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for V2 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let version = String::deserialize(deserializer)?;
        if version == "2.0" {
            Ok(Self)
        } else {
            Err(de::Error::custom("jsonrpc must be \"2.0\""))
        }
    }
}

/// A request id. A request without one is a notification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Number(i64),
    String(String),
}

/// A call, or a notification when `id` is absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub jsonrpc: V2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Request {
    pub fn call(id: Id, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: V2,
            id: Some(id),
            method: method.into(),
            params: Some(params),
        }
    }

    pub fn notification(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: V2,
            id: None,
            method: method.into(),
            params: Some(params),
        }
    }
}

/// The answer to a call. `id` is null only when the request was too broken
/// to read one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: V2,
    pub id: Option<Id>,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Result(Value),
    Error(RpcError),
}

impl Response {
    pub fn ok(id: Option<Id>, result: Value) -> Self {
        Self {
            jsonrpc: V2,
            id,
            outcome: Outcome::Result(result),
        }
    }

    pub fn err(id: Option<Id>, error: RpcError) -> Self {
        Self {
            jsonrpc: V2,
            id,
            outcome: Outcome::Error(error),
        }
    }
}

/// A JSON-RPC error, always with an [`ApiError`] as its data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    /// The kind, as a short word; the plain message is in `data`.
    pub message: String,
    pub data: ApiError,
}

impl RpcError {
    /// An application error, coded by its kind.
    pub fn from_api(error: ApiError) -> Self {
        Self {
            code: code_for(error.kind),
            message: error.kind.as_str().to_owned(),
            data: error,
        }
    }

    fn standard(code: i64, kind: ErrorKind, user_message: &str, detail: String) -> Self {
        Self {
            code,
            message: kind.as_str().to_owned(),
            data: ApiError::new(kind, user_message).with_detail(detail),
        }
    }

    pub fn parse_error(detail: impl Into<String>) -> Self {
        Self::standard(
            code::PARSE_ERROR,
            ErrorKind::InvalidInput,
            "PriyaOS couldn't read that request.",
            detail.into(),
        )
    }

    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self::standard(
            code::INVALID_REQUEST,
            ErrorKind::InvalidInput,
            "PriyaOS couldn't read that request.",
            detail.into(),
        )
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::standard(
            code::METHOD_NOT_FOUND,
            ErrorKind::NotFound,
            "This version of PriyaOS doesn't know that request.",
            format!("no method named {method}"),
        )
    }

    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self::standard(
            code::INVALID_PARAMS,
            ErrorKind::InvalidInput,
            "That request has a wrong or missing value.",
            detail.into(),
        )
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::standard(
            code::INTERNAL_ERROR,
            ErrorKind::Internal,
            "Something went wrong inside PriyaOS.",
            detail.into(),
        )
    }
}

/// One line read from a connection: a single message or a batch.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Single(Value),
    Batch(Vec<Value>),
}

/// Reads one line of JSON. Each message in it still has to be checked with
/// [`parse_request`].
pub fn parse_line(line: &[u8]) -> Result<Incoming, RpcError> {
    match serde_json::from_slice::<Value>(line) {
        Ok(Value::Array(items)) if items.is_empty() => {
            Err(RpcError::invalid_request("an empty batch"))
        }
        Ok(Value::Array(items)) => Ok(Incoming::Batch(items)),
        Ok(value) => Ok(Incoming::Single(value)),
        Err(e) => Err(RpcError::parse_error(e.to_string())),
    }
}

/// Checks one message. On failure, returns the id if one could be read, so
/// the error can still be answered to it.
pub fn parse_request(value: Value) -> Result<Request, (Option<Id>, RpcError)> {
    let id = value
        .get("id")
        .and_then(|id| serde_json::from_value::<Id>(id.clone()).ok());
    serde_json::from_value(value).map_err(|e| (id, RpcError::invalid_request(e.to_string())))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // covers: CORE-04
    // suite: unit
    // acceptance: 0.7
    #[test]
    fn errors_carry_their_kind_code_and_plain_message() {
        let error = RpcError::from_api(ApiError::new(ErrorKind::NotFound, "No such setting."));
        let response = Response::err(Some(Id::Number(7)), error);
        assert_eq!(
            serde_json::to_value(&response).unwrap(),
            json!({
                "jsonrpc": "2.0",
                "id": 7,
                "error": {
                    "code": -32001,
                    "message": "not_found",
                    "data": {"kind": "not_found", "retryable": false, "user_message": "No such setting."}
                }
            })
        );
        let codes: Vec<i64> = [
            ErrorKind::NotFound,
            ErrorKind::InvalidInput,
            ErrorKind::Conflict,
            ErrorKind::PermissionDenied,
            ErrorKind::Unavailable,
            ErrorKind::Timeout,
            ErrorKind::Cancelled,
            ErrorKind::ProviderError,
            ErrorKind::Internal,
        ]
        .into_iter()
        .map(code_for)
        .collect();
        assert_eq!(
            codes,
            [
                -32001, -32002, -32003, -32004, -32005, -32006, -32007, -32008, -32603
            ]
        );
        assert_eq!(RpcError::method_not_found("v1.x").code, -32601);
        assert_eq!(RpcError::invalid_params("x").code, -32602);
        assert_eq!(RpcError::parse_error("x").code, -32700);
        assert_eq!(RpcError::invalid_request("x").code, -32600);
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn requests_and_responses_round_trip() {
        let call = Request::call(Id::String("a".into()), "v1.system.status", json!({}));
        let text = serde_json::to_string(&call).unwrap();
        assert_eq!(
            text,
            r#"{"jsonrpc":"2.0","id":"a","method":"v1.system.status","params":{}}"#
        );
        assert_eq!(
            parse_request(serde_json::from_str(&text).unwrap()),
            Ok(call)
        );

        let ok: Response =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":{"x":1}}"#).unwrap();
        assert_eq!(ok.outcome, Outcome::Result(json!({"x": 1})));
        let null_id =
            serde_json::to_value(Response::err(None, RpcError::parse_error("x"))).unwrap();
        assert_eq!(null_id["id"], Value::Null);
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn malformed_messages_are_refused_with_the_id_when_readable() {
        assert!(matches!(parse_line(b"{not json"), Err(e) if e.code == -32700));
        assert!(matches!(parse_line(b"[]"), Err(e) if e.code == -32600));
        assert!(
            matches!(parse_line(br#"[{"a":1},{"b":2}]"#), Ok(Incoming::Batch(items)) if items.len() == 2)
        );

        let wrong_version = json!({"jsonrpc": "1.0", "id": 3, "method": "x"});
        let (id, error) = parse_request(wrong_version).unwrap_err();
        assert_eq!((id, error.code), (Some(Id::Number(3)), -32600));
        let no_method = json!({"jsonrpc": "2.0", "id": "q"});
        assert_eq!(
            parse_request(no_method).unwrap_err().0,
            Some(Id::String("q".into()))
        );
        let notification = parse_request(json!({"jsonrpc": "2.0", "method": "x"})).unwrap();
        assert_eq!(notification.id, None);
    }
}
