//! JSON-RPC 2.0 messages (02-core §4), and how [`ApiError`]s travel in them.
//!
//! Every error carries `data: {kind, retryable, user_message}` (an
//! [`ApiError`]), so a client can show the plain message and branch on the
//! kind without parsing text. The standard codes cover malformed traffic;
//! application errors use one code per [`ErrorKind`].
//!
//! Requests refuse members JSON-RPC doesn't define, so adding one later is
//! a deliberate API change.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Number, Value};

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

/// The most messages one batch may hold.
pub const MAX_BATCH: usize = 1024;

/// Every kind, for going from a code back to its kind.
const KINDS: [ErrorKind; 9] = [
    ErrorKind::NotFound,
    ErrorKind::InvalidInput,
    ErrorKind::Conflict,
    ErrorKind::PermissionDenied,
    ErrorKind::Unavailable,
    ErrorKind::Timeout,
    ErrorKind::Cancelled,
    ErrorKind::ProviderError,
    ErrorKind::Internal,
];

/// The application error code for each kind. Exhaustive, so a new kind
/// can't go without one.
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

/// The kind an error code stands for, for errors that come without `data`.
pub fn kind_for(code: i64) -> ErrorKind {
    match code {
        code::PARSE_ERROR | code::INVALID_REQUEST | code::INVALID_PARAMS => ErrorKind::InvalidInput,
        code::METHOD_NOT_FOUND => ErrorKind::NotFound,
        _ => KINDS
            .into_iter()
            .find(|kind| code_for(*kind) == code)
            .unwrap_or(ErrorKind::Internal),
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

/// A request id: any JSON number or string, or null. A request without an
/// id at all is a notification; one with a null id is a call.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Number(Number),
    String(String),
    Null,
}

impl From<i64> for Id {
    fn from(n: i64) -> Self {
        Self::Number(n.into())
    }
}

/// `Some` for any id that is present, null included; `None` (through
/// `#[serde(default)]`) only when the member is missing.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Id>, D::Error> {
    Id::deserialize(deserializer).map(Some)
}

/// A call, or a notification when `id` is absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub jsonrpc: V2,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub id: Option<Id>,
    pub method: String,
    /// Named params (an object); positional ones (an array) are refused by
    /// the methods, which all take named params.
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

/// The answer to a call. `id` is null when the request was too broken to
/// read one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Response {
    pub jsonrpc: V2,
    pub id: Id,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Result(Value),
    Error(RpcError),
}

impl Response {
    pub fn ok(id: Id, result: Value) -> Self {
        Self {
            jsonrpc: V2,
            id,
            outcome: Outcome::Result(result),
        }
    }

    pub fn err(id: Id, error: RpcError) -> Self {
        Self {
            jsonrpc: V2,
            id,
            outcome: Outcome::Error(error),
        }
    }
}

impl<'de> Deserialize<'de> for Response {
    /// Exactly one of `result` (which may be null) and `error`.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            jsonrpc: V2,
            id: Id,
            #[serde(default, deserialize_with = "some_value")]
            result: Option<Value>,
            #[serde(default)]
            error: Option<RpcError>,
        }
        fn some_value<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
            Value::deserialize(d).map(Some)
        }
        let raw = Raw::deserialize(deserializer)?;
        let outcome = match (raw.result, raw.error) {
            (Some(result), None) => Outcome::Result(result),
            (None, Some(error)) => Outcome::Error(error),
            (Some(_), Some(_)) => {
                return Err(de::Error::custom(
                    "a response has both a result and an error",
                ));
            }
            (None, None) => {
                return Err(de::Error::custom(
                    "a response has neither a result nor an error",
                ));
            }
        };
        Ok(Self {
            jsonrpc: raw.jsonrpc,
            id: raw.id,
            outcome,
        })
    }
}

/// A JSON-RPC error, always with an [`ApiError`] as its data.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RpcError {
    pub code: i64,
    /// The standard message for standard codes, else the kind.
    pub message: String,
    pub data: ApiError,
}

impl<'de> Deserialize<'de> for RpcError {
    /// An error from elsewhere (a module) may lack `data`: one is made up
    /// from the code, with a plain message; the peer's own message, which
    /// may be technical, becomes the detail.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            code: i64,
            message: String,
            #[serde(default)]
            data: Option<ApiError>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let data = raw.data.unwrap_or_else(|| {
            ApiError::new(
                kind_for(raw.code),
                "Something PriyaOS asked for didn't work.",
            )
            .with_detail(raw.message.clone())
        });
        Ok(Self {
            code: raw.code,
            message: raw.message,
            data,
        })
    }
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

    fn standard(code: i64, message: &str, user_message: &str, detail: String) -> Self {
        Self {
            code,
            message: message.to_owned(),
            data: ApiError::new(kind_for(code), user_message).with_detail(detail),
        }
    }

    pub fn parse_error(detail: impl Into<String>) -> Self {
        Self::standard(
            code::PARSE_ERROR,
            "Parse error",
            "PriyaOS couldn't read that request.",
            detail.into(),
        )
    }

    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self::standard(
            code::INVALID_REQUEST,
            "Invalid Request",
            "PriyaOS couldn't read that request.",
            detail.into(),
        )
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::standard(
            code::METHOD_NOT_FOUND,
            "Method not found",
            "This version of PriyaOS doesn't know that request.",
            format!("no method named {method}"),
        )
    }

    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self::standard(
            code::INVALID_PARAMS,
            "Invalid params",
            "That request has a wrong or missing value.",
            detail.into(),
        )
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::standard(
            code::INTERNAL_ERROR,
            "Internal error",
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
        Ok(Value::Array(items)) if items.len() > MAX_BATCH => Err(RpcError::invalid_request(
            format!("a batch of {} messages; at most {MAX_BATCH}", items.len()),
        )),
        Ok(Value::Array(items)) => Ok(Incoming::Batch(items)),
        Ok(value) => Ok(Incoming::Single(value)),
        Err(e) => Err(RpcError::parse_error(e.to_string())),
    }
}

/// Checks one message. On failure, returns the id to answer the error to:
/// the message's own if it could be read, else null.
pub fn parse_request(value: Value) -> Result<Request, (Id, RpcError)> {
    let id = value
        .get("id")
        .and_then(|id| serde_json::from_value::<Id>(id.clone()).ok())
        .unwrap_or(Id::Null);
    let request: Request = serde_json::from_value(value)
        .map_err(|e| (id.clone(), RpcError::invalid_request(e.to_string())))?;
    match &request.params {
        None | Some(Value::Object(_) | Value::Array(_)) => Ok(request),
        Some(_) => Err((
            id,
            RpcError::invalid_request("params must be an object or an array"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // covers: CORE-04
    // suite: unit
    #[test]
    fn errors_carry_their_kind_code_and_plain_message() {
        let error = RpcError::from_api(ApiError::new(ErrorKind::NotFound, "No such setting."));
        let response = Response::err(Id::from(7), error);
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
        for kind in KINDS {
            assert_eq!(kind_for(code_for(kind)), kind);
        }
        assert_eq!(code_for(ErrorKind::Conflict), -32003);
        assert_eq!(RpcError::method_not_found("v1.x").code, -32601);
        assert_eq!(RpcError::invalid_params("x").message, "Invalid params");
        assert_eq!(RpcError::parse_error("x").code, -32700);
        assert_eq!(
            RpcError::invalid_request("x").data.kind,
            ErrorKind::InvalidInput
        );
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
        let null_result: Response =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":null}"#).unwrap();
        assert_eq!(null_result.outcome, Outcome::Result(Value::Null));
        let null_id =
            serde_json::to_value(Response::err(Id::Null, RpcError::parse_error("x"))).unwrap();
        assert_eq!(null_id["id"], Value::Null);
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn ids_follow_the_json_rpc_rules() {
        // Null is a call, not a notification.
        let null = parse_request(json!({"jsonrpc": "2.0", "id": null, "method": "x"})).unwrap();
        assert_eq!(null.id, Some(Id::Null));
        let absent = parse_request(json!({"jsonrpc": "2.0", "method": "x"})).unwrap();
        assert_eq!(absent.id, None);
        // Any number is kept exactly, so errors still reach the caller.
        for id in [json!(1.5), json!(18446744073709551615_u64), json!(-3)] {
            let request =
                parse_request(json!({"jsonrpc": "2.0", "id": id, "method": "x"})).unwrap();
            assert_eq!(serde_json::to_value(request.id.unwrap()).unwrap(), id);
        }
        let (id, _) =
            parse_request(json!({"jsonrpc": "1.0", "id": 2.5, "method": "x"})).unwrap_err();
        assert_eq!(serde_json::to_value(id).unwrap(), json!(2.5));
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
        let huge = format!("[{}1]", "1,".repeat(MAX_BATCH));
        assert!(matches!(parse_line(huge.as_bytes()), Err(e) if e.code == -32600));

        let wrong_version = json!({"jsonrpc": "1.0", "id": 3, "method": "x"});
        let (id, error) = parse_request(wrong_version).unwrap_err();
        assert_eq!((id, error.code), (Id::from(3), -32600));
        let no_method = json!({"jsonrpc": "2.0", "id": "q"});
        assert_eq!(
            parse_request(no_method).unwrap_err().0,
            Id::String("q".into())
        );
        let scalar = json!({"jsonrpc": "2.0", "id": 4, "method": "x", "params": 5});
        assert_eq!(parse_request(scalar).unwrap_err().1.code, -32600);
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn responses_need_exactly_one_outcome_and_errors_may_lack_data() {
        let both = serde_json::from_str::<Response>(
            r#"{"jsonrpc":"2.0","id":1,"result":1,"error":{"code":1,"message":"x"}}"#,
        );
        assert!(both.unwrap_err().to_string().contains("both"));
        let neither = serde_json::from_str::<Response>(r#"{"jsonrpc":"2.0","id":1}"#);
        assert!(neither.unwrap_err().to_string().contains("neither"));

        let bare: Response = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#,
        )
        .unwrap();
        let Outcome::Error(error) = bare.outcome else {
            panic!("an error")
        };
        assert_eq!(error.data.kind, ErrorKind::NotFound);
        // The peer's text may be technical: it is the detail, not the
        // message shown.
        assert_eq!(
            error.data.user_message,
            "Something PriyaOS asked for didn't work."
        );
        assert_eq!(error.data.detail.as_deref(), Some("Method not found"));
    }
}
