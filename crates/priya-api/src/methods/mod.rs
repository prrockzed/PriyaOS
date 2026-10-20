//! Every API method: its name, its params and result types, and its JSON
//! Schemas (02-core §4).
//!
//! [`ALL`] lists them, for `v1.system.methods`, the CLI parity test and the
//! UI's generated helpers (0.9). A method's types live in its group's module.

use schemars::{JsonSchema, Schema};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::rpc::RpcError;

pub mod audit;
pub mod events;
pub mod settings;
pub mod system;

/// One API method.
pub trait Method {
    /// The wire name, such as `v1.system.status`.
    const NAME: &'static str;
    /// One plain sentence: what it does.
    const SUMMARY: &'static str;
    /// Whether it only reads: it changes nothing a later request would see
    /// (modules may call only these, 0.8).
    const READ_ONLY: bool;
    /// The notification it sends until stopped, if it is a subscription.
    const NOTIFICATION: Option<&'static str> = None;
    type Params: Serialize + DeserializeOwned + JsonSchema;
    type Result: Serialize + DeserializeOwned + JsonSchema;
}

/// A method as data, for listing and introspection.
#[derive(Debug, Clone, Copy)]
pub struct MethodInfo {
    pub name: &'static str,
    pub summary: &'static str,
    pub read_only: bool,
    pub notification: Option<&'static str>,
    pub params: fn() -> Schema,
    pub result: fn() -> Schema,
}

impl MethodInfo {
    const fn of<M: Method>() -> Self {
        Self {
            name: M::NAME,
            summary: M::SUMMARY,
            read_only: M::READ_ONLY,
            notification: M::NOTIFICATION,
            params: schema::<M::Params>,
            result: schema::<M::Result>,
        }
    }
}

fn schema<T: JsonSchema>() -> Schema {
    schemars::schema_for!(T)
}

/// Every method of this API version.
pub const ALL: &[MethodInfo] = &[
    MethodInfo::of::<system::Status>(),
    MethodInfo::of::<system::Methods>(),
    MethodInfo::of::<system::Doctor>(),
    MethodInfo::of::<settings::Get>(),
    MethodInfo::of::<settings::Set>(),
    MethodInfo::of::<settings::Unset>(),
    MethodInfo::of::<settings::GetSchema>(),
    MethodInfo::of::<events::Subscribe>(),
    MethodInfo::of::<events::Unsubscribe>(),
    MethodInfo::of::<audit::Verify>(),
];

/// A notification the daemon sends.
#[derive(Debug, Clone, Copy)]
pub struct NotificationInfo {
    pub name: &'static str,
    pub params: fn() -> Schema,
}

/// Every notification the daemon sends.
pub const NOTIFICATIONS: &[NotificationInfo] = &[NotificationInfo {
    name: events::EVENT_NOTIFICATION,
    params: schema::<events::EventNotification>,
}];

/// A method's params from a request: absent or null means none (`{}`);
/// params are named, so a list is refused.
pub fn decode_params<P: DeserializeOwned>(params: Option<Value>) -> Result<P, RpcError> {
    let params = match params {
        None | Some(Value::Null) => Value::Object(Map::new()),
        Some(object @ Value::Object(_)) => object,
        Some(_) => return Err(RpcError::invalid_params("params must be named (an object)")),
    };
    serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))
}

/// Params of a method that takes none: `{}`, or no params at all (see
/// [`decode_params`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// The result of a method that only reports success.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize, JsonSchema)]
pub struct Done {}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    // covers: CORE-04
    // suite: unit
    #[test]
    fn every_method_is_listed_once_with_schemas() {
        let names: HashSet<&str> = ALL.iter().map(|m| m.name).collect();
        assert_eq!(names.len(), ALL.len(), "no duplicates");
        for method in ALL {
            assert!(method.name.starts_with("v1."), "{}", method.name);
            assert!(method.summary.ends_with('.'), "{}", method.name);
            let params = serde_json::to_value((method.params)()).unwrap();
            let result = serde_json::to_value((method.result)()).unwrap();
            assert!(params.is_object() && result.is_object(), "{}", method.name);
        }
        // Every notification a method names is listed, with its schema.
        for method in ALL.iter().filter_map(|m| m.notification) {
            assert!(NOTIFICATIONS.iter().any(|n| n.name == method), "{method}");
        }
        assert!(
            ALL.iter()
                .any(|m| m.name == "v1.events.subscribe" && m.notification.is_some())
        );
        assert!(
            !ALL.iter()
                .any(|m| m.name == "v1.settings.set" && m.read_only)
        );
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn params_are_named_and_may_be_left_out() {
        use serde_json::json;
        assert_eq!(decode_params::<NoParams>(None).unwrap(), NoParams {});
        assert_eq!(
            decode_params::<NoParams>(Some(Value::Null)).unwrap(),
            NoParams {}
        );
        let get: settings::GetParams = decode_params(Some(json!({"key": "ui.theme"}))).unwrap();
        assert_eq!(get.key.as_deref(), Some("ui.theme"));
        let by_position = decode_params::<settings::GetParams>(Some(json!(["ui.theme", true])));
        assert_eq!(by_position.unwrap_err().code, -32602);
        let unknown = decode_params::<NoParams>(Some(json!({"x": 1})));
        assert_eq!(unknown.unwrap_err().code, -32602);
    }
}
