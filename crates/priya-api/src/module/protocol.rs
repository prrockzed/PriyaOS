//! The module protocol, v1 (02-core §5.2): JSON-RPC 2.0 between priyad and
//! a module, both ways, over stdio with `Content-Length` framing (or
//! in-process channels for a builtin module).
//!
//! The host calls the module: [`Initialize`] once, then [`Ping`] now and
//! then, the module's own methods (`<module>.<method>`) and finally
//! [`Shutdown`]; it also sends the events the module subscribes to, one
//! [`DeliverEvent`] call at a time, so a slow module slows delivery down
//! instead of falling behind. The module calls the host: [`EmitEvent`],
//! [`GetSetting`] and [`CallApi`], each checked against its manifest.
//!
//! What the host sends a module accepts fields it doesn't know, so a newer
//! priyad can add some without breaking modules built against an older
//! SDK. What a module sends the host is closed, so mistakes show.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::methods::events::StoredEvent;
use crate::methods::{Done, Method, MethodInfo};

/// `initialize`: the first call. The module answers with who it is, which
/// must match its manifest.
pub struct Initialize;

impl Method for Initialize {
    const NAME: &'static str = "initialize";
    const SUMMARY: &'static str = "Starts the session and asks the module who it is.";
    const READ_ONLY: bool = true;
    type Params = InitializeParams;
    type Result = InitializeResult;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InitializeParams {
    /// The module API version the host speaks: `v1`.
    pub api: String,
    /// priyad's version.
    pub host_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InitializeResult {
    /// The module API version the module speaks: `v1`.
    pub api: String,
    /// The manifest's `module.name`.
    pub name: String,
    /// The manifest's `module.version`.
    pub version: String,
    /// The manifest's `provides.methods`, in any order.
    pub methods: Vec<String>,
}

impl InitializeResult {
    /// The answer of a module speaking API v1.
    pub fn v1<S: Into<String>>(
        name: impl Into<String>,
        version: impl Into<String>,
        methods: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            api: crate::module::API_VERSION.to_owned(),
            name: name.into(),
            version: version.into(),
            methods: methods.into_iter().map(Into::into).collect(),
        }
    }
}

/// `ping`: are you still there?
pub struct Ping;

impl Method for Ping {
    const NAME: &'static str = "ping";
    const SUMMARY: &'static str = "Checks that the module still answers.";
    const READ_ONLY: bool = true;
    type Params = Empty;
    type Result = Done;
}

/// Params of a host-to-module call that takes none. Unlike
/// [`NoParams`](crate::methods::NoParams), unknown fields are allowed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Empty {}

/// `shutdown`: finish up and exit. The module answers, then exits.
pub struct Shutdown;

impl Method for Shutdown {
    const NAME: &'static str = "shutdown";
    const SUMMARY: &'static str = "Asks the module to finish up and exit.";
    const READ_ONLY: bool = false;
    type Params = Empty;
    type Result = Done;
}

/// `event`: an event the module subscribes to (`[subscribes] events`).
/// The host waits for the answer before sending the next one.
pub struct DeliverEvent;

impl Method for DeliverEvent {
    const NAME: &'static str = "event";
    const SUMMARY: &'static str = "Hands the module an event it subscribes to.";
    const READ_ONLY: bool = true;
    type Params = EventDelivery;
    type Result = Done;
}

/// [`DeliverEvent`]'s params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventDelivery {
    /// The event, with its full payload.
    pub event: StoredEvent,
}

/// `host.emit_event`: record one of the module's own events, as
/// `module.event` with actor `module:<name>`. Needs `events.emit`.
pub struct EmitEvent;

impl Method for EmitEvent {
    const NAME: &'static str = "host.emit_event";
    const SUMMARY: &'static str = "Records one of the module's own events.";
    const READ_ONLY: bool = false;
    type Params = EmitEventParams;
    type Result = EmitEventResult;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmitEventParams {
    /// The module's name for the event, such as `forecast.updated`:
    /// lowercase words joined by dots.
    #[schemars(extend("pattern" = CUSTOM_TYPE_PATTERN, "maxLength" = 64))]
    pub custom_type: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EmitEventResult {
    /// Where the event sits in the log.
    pub seq: i64,
}

/// `host.settings.get`: one setting's value. A module may always read its
/// own `modules.<name>` section; other keys need `settings.read:<prefix>`.
pub struct GetSetting;

impl Method for GetSetting {
    const NAME: &'static str = "host.settings.get";
    const SUMMARY: &'static str = "Reads a setting the module may see.";
    const READ_ONLY: bool = true;
    type Params = GetSettingParams;
    type Result = GetSettingResult;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetSettingParams {
    /// A dotted key, such as `modules.echo.prefix` or `ui.theme`.
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetSettingResult {
    /// The value in force; null when it isn't set.
    pub value: Value,
}

/// `host.call`: a read-only public API method, such as `v1.system.status`.
/// Needs `api.read`.
pub struct CallApi;

impl Method for CallApi {
    const NAME: &'static str = "host.call";
    const SUMMARY: &'static str = "Calls a read-only public API method.";
    const READ_ONLY: bool = true;
    type Params = CallApiParams;
    type Result = Value;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CallApiParams {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// The methods every module answers, for schemas (0.9).
pub const MODULE_METHODS: &[MethodInfo] = &[
    MethodInfo::of::<Initialize>(),
    MethodInfo::of::<Ping>(),
    MethodInfo::of::<Shutdown>(),
    MethodInfo::of::<DeliverEvent>(),
];

/// The methods the host answers for modules, for schemas (0.9).
pub const HOST_METHODS: &[MethodInfo] = &[
    MethodInfo::of::<EmitEvent>(),
    MethodInfo::of::<GetSetting>(),
    MethodInfo::of::<CallApi>(),
];

const CUSTOM_TYPE_PATTERN: &str = "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)*$";

/// Whether `name` may name a module's event: lowercase words of letters,
/// digits and `_` joined by dots, at most 64 bytes.
pub fn is_custom_type(name: &str) -> bool {
    name.len() <= 64
        && name.split('.').all(|word| {
            let bytes = word.as_bytes();
            !bytes.is_empty()
                && bytes[0].is_ascii_lowercase()
                && bytes
                    .iter()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-02
    // suite: unit
    #[test]
    fn protocol_methods_are_listed_with_schemas() {
        for method in MODULE_METHODS.iter().chain(HOST_METHODS) {
            assert!(method.summary.ends_with('.'), "{}", method.name);
            assert!((method.params)().as_object().is_some(), "{}", method.name);
        }
        assert!(HOST_METHODS.iter().all(|m| m.name.starts_with("host.")));
        assert!(MODULE_METHODS.iter().all(|m| !m.name.contains('.')));

        for good in ["updated", "forecast.updated", "a1.b_2"] {
            assert!(is_custom_type(good), "{good}");
        }
        for bad in [
            "",
            "Updated",
            "a..b",
            ".a",
            "a.",
            "1a",
            "a-b",
            &"a".repeat(65),
        ] {
            assert!(!is_custom_type(bad), "{bad}");
        }
        let params: EmitEventParams =
            serde_json::from_value(serde_json::json!({"custom_type": "said"})).unwrap();
        assert_eq!(params.payload, Value::Null, "the payload may be left out");

        // A newer host may send more; a module may not.
        let newer = serde_json::json!({"api": "v1", "host_version": "9.0.0", "later": true});
        assert!(serde_json::from_value::<InitializeParams>(newer).is_ok());
        assert!(serde_json::from_value::<Empty>(serde_json::json!({"later": 1})).is_ok());
        let extra = serde_json::json!({"custom_type": "said", "later": 1});
        assert!(serde_json::from_value::<EmitEventParams>(extra).is_err());
        let schema = serde_json::to_value(schemars::schema_for!(EmitEventParams)).unwrap();
        assert_eq!(schema["properties"]["custom_type"]["maxLength"], 64);
    }
}
