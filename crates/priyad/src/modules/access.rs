//! What a module may ask of priyad: every host call is checked against the
//! capabilities in its manifest before anything happens (02-core §5.2).

use priya_api::error::{ApiError, ErrorKind};
use priya_api::methods::{Method, settings, system};
use priya_api::module::{Capability, ModuleManifest};
use priya_core::settings::KeyPath;
use serde_json::Value;

/// The public methods `host.call` may reach with `api.read`. A method is
/// added here on purpose, never just for being read-only: a module must
/// not get around its other limits through the public API.
pub const CALLABLE: &[&str] = &[
    system::Status::NAME,
    system::Methods::NAME,
    settings::GetSchema::NAME,
    // Only for one key the module may read; see `check`.
    settings::Get::NAME,
];

/// A host call, as far as permission goes.
#[derive(Debug, Clone, Copy)]
pub enum HostCall<'a> {
    /// `host.emit_event`
    EmitEvent,
    /// `host.settings.get`
    ReadSetting(&'a KeyPath),
    /// `host.call` of a public method with its params.
    CallApi { method: &'a str, params: &'a Value },
    /// `host.ai.ask` with this role.
    AskAi(&'a str),
}

/// Whether `manifest` allows `call`; a refusal says why in plain words.
pub fn check(manifest: &ModuleManifest, call: HostCall<'_>) -> Result<(), ApiError> {
    let name = manifest.name();
    match call {
        HostCall::EmitEvent if manifest.grants(&Capability::EventsEmit) => Ok(()),
        HostCall::EmitEvent => Err(denied(name, "record events", "events.emit")),
        HostCall::ReadSetting(key) => read_setting(manifest, key),
        HostCall::AskAi(role) => {
            if manifest.grants(&Capability::AiRole(role.to_owned())) {
                Ok(())
            } else {
                Err(denied(name, "ask Priya AI", &format!("ai.role:{role}")))
            }
        }
        HostCall::CallApi { method, params } => {
            if !CALLABLE.contains(&method) {
                return Err(ApiError::new(
                    ErrorKind::PermissionDenied,
                    format!("The {name} module can't make the request `{method}`."),
                )
                .with_detail(format!(
                    "modules may call only {}; events come through [subscribes] in module.toml",
                    CALLABLE.join(", ")
                )));
            }
            if !manifest.grants(&Capability::ApiRead) {
                return Err(denied(name, &format!("call `{method}`"), "api.read"));
            }
            if method == settings::Get::NAME {
                // The same limits as host.settings.get: one key at a time.
                let key = params
                    .get("key")
                    .and_then(Value::as_str)
                    .and_then(|key| KeyPath::parse(key).ok())
                    .ok_or_else(|| {
                        ApiError::new(
                            ErrorKind::PermissionDenied,
                            format!("The {name} module may read settings only one key at a time."),
                        )
                    })?;
                return read_setting(manifest, &key);
            }
            Ok(())
        }
    }
}

fn read_setting(manifest: &ModuleManifest, key: &KeyPath) -> Result<(), ApiError> {
    if manifest.may_read_setting(key.parts()) {
        Ok(())
    } else {
        Err(denied(
            manifest.name(),
            &format!("read the setting `{key}`"),
            &format!("settings.read:{}", key.parts().join(".")),
        ))
    }
}

fn denied(module: &str, what: &str, capability: &str) -> ApiError {
    ApiError::new(
        ErrorKind::PermissionDenied,
        format!("The {module} module isn't allowed to {what}."),
    )
    .with_detail(format!(
        "its module.toml would need the capability `{capability}`"
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn manifest(capabilities: &str) -> ModuleManifest {
        ModuleManifest::parse(&format!(
            "[module]\nname = \"echo\"\nversion = \"0.1.0\"\napi = \"v1\"\nkind = \"builtin\"\n\
             [requires]\ncapabilities = [{capabilities}]\n"
        ))
        .unwrap()
    }

    fn key(text: &str) -> KeyPath {
        KeyPath::parse(text).unwrap()
    }

    fn call<'a>(method: &'a str, params: &'a Value) -> HostCall<'a> {
        HostCall::CallApi { method, params }
    }

    fn refused(manifest: &ModuleManifest, call: HostCall<'_>) -> ApiError {
        check(manifest, call).unwrap_err()
    }

    // covers: CORE-02
    // suite: unit
    // acceptance: 0.8
    // regression: 0.8
    #[test]
    fn capability_check_denies_undeclared_host_call() {
        let none = json!({});
        let bare = manifest("");
        let error = refused(&bare, HostCall::EmitEvent);
        assert_eq!(error.kind, ErrorKind::PermissionDenied);
        assert_eq!(
            error.user_message,
            "The echo module isn't allowed to record events."
        );
        assert!(error.detail.unwrap().contains("events.emit"));
        let error = refused(&bare, call("v1.system.status", &none));
        assert!(error.detail.unwrap().contains("api.read"));
        let theme = key("ui.theme");
        let error = refused(&bare, HostCall::ReadSetting(&theme));
        assert!(error.detail.unwrap().contains("settings.read:ui.theme"));
        // Its own section needs nothing; another module's is out of reach,
        // and the hint names just that key.
        assert!(check(&bare, HostCall::ReadSetting(&key("modules.echo.prefix"))).is_ok());
        let other = key("modules.weather.city");
        let error = refused(&bare, HostCall::ReadSetting(&other));
        assert!(
            error
                .detail
                .unwrap()
                .contains("settings.read:modules.weather.city")
        );

        // Declared, they are allowed, but only within what they say.
        let granted = manifest("\"events.emit\", \"api.read\", \"settings.read:ui\"");
        assert!(check(&granted, HostCall::EmitEvent).is_ok());
        assert!(check(&granted, call("v1.system.status", &none)).is_ok());
        assert!(check(&granted, HostCall::ReadSetting(&theme)).is_ok());
        assert!(check(&granted, HostCall::ReadSetting(&key("ai.strategy"))).is_err());
    }

    // covers: CORE-02
    // suite: security
    #[test]
    fn api_read_reaches_only_the_listed_methods_and_settings_the_module_may_read() {
        let reader = manifest("\"api.read\", \"settings.read:ui\"");
        let none = json!({});
        // Nothing that changes things, no streams, nothing heavy or odd,
        // and nothing that doesn't exist.
        for method in [
            "v1.settings.set",
            "v1.events.subscribe",
            "v1.events.unsubscribe",
            "v1.audit.verify",
            "v1.system.doctor",
            "v1.nope",
        ] {
            let error = refused(&reader, call(method, &none));
            assert_eq!(error.kind, ErrorKind::PermissionDenied, "{method}");
            assert!(error.user_message.contains("echo"), "{method}");
        }
        // v1.settings.get: one key it may read, never everything.
        let get = settings::Get::NAME;
        assert!(check(&reader, call(get, &json!({"key": "ui.theme"}))).is_ok());
        assert!(check(&reader, call(get, &json!({"key": "modules.echo.prefix"}))).is_ok());
        for params in [
            json!({}),
            json!({"key": "ai.strategy"}),
            json!({"key": "modules.weather"}),
            json!({"key": 5}),
            json!({"key": "not a key!"}),
        ] {
            let error = refused(&reader, call(get, &params));
            assert_eq!(error.kind, ErrorKind::PermissionDenied, "{params}");
        }
    }
}
