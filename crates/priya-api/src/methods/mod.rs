//! Every API method: its name, its params and result types, and its JSON
//! Schemas (02-core §4).
//!
//! [`ALL`] lists them, for `v1.system.methods`, the CLI parity test and the
//! UI's generated helpers (0.9). A method's types live in its group's module.

use schemars::{JsonSchema, Schema};
use serde::Serialize;
use serde::de::DeserializeOwned;

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
    /// Whether it also sends notifications until stopped (a subscription).
    const STREAMS: bool = false;
    type Params: Serialize + DeserializeOwned + JsonSchema;
    type Result: Serialize + DeserializeOwned + JsonSchema;
}

/// A method as data, for listing and introspection.
#[derive(Debug, Clone, Copy)]
pub struct MethodInfo {
    pub name: &'static str,
    pub summary: &'static str,
    pub streams: bool,
    pub params: fn() -> Schema,
    pub result: fn() -> Schema,
}

impl MethodInfo {
    const fn of<M: Method>() -> Self {
        Self {
            name: M::NAME,
            summary: M::SUMMARY,
            streams: M::STREAMS,
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

/// Notifications the daemon sends, by name.
pub const NOTIFICATIONS: &[&str] = &[events::EVENT_NOTIFICATION];

/// Params of a method that takes none: `{}` (or no params at all).
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
        assert!(
            ALL.iter()
                .any(|m| m.name == "v1.events.subscribe" && m.streams)
        );
    }
}
