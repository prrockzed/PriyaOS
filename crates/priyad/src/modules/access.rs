//! What a module may ask of priyad: every host call is checked against the
//! capabilities in its manifest before anything happens (02-core §5.2).

use priya_api::error::{ApiError, ErrorKind};
use priya_api::methods::ALL;
use priya_api::module::{Capability, ModuleManifest};
use priya_core::settings::KeyPath;

/// A host call, as far as permission goes.
#[derive(Debug, Clone, Copy)]
pub enum HostCall<'a> {
    /// `host.emit_event`
    EmitEvent,
    /// `host.settings.get`
    ReadSetting(&'a KeyPath),
    /// `host.call` of a public method.
    CallApi(&'a str),
}

/// Whether `manifest` allows `call`; a refusal says why in plain words.
pub fn check(manifest: &ModuleManifest, call: HostCall<'_>) -> Result<(), ApiError> {
    let name = manifest.name();
    match call {
        HostCall::EmitEvent if manifest.grants(&Capability::EventsEmit) => Ok(()),
        HostCall::EmitEvent => Err(denied(name, "record events", "events.emit")),
        HostCall::ReadSetting(key) if manifest.may_read_setting(key.parts()) => Ok(()),
        HostCall::ReadSetting(key) => Err(denied(
            name,
            &format!("read the setting `{key}`"),
            &format!(
                "settings.read:{}",
                key.parts().first().map_or("", String::as_str)
            ),
        )),
        HostCall::CallApi(method) => {
            let Some(info) = ALL.iter().find(|m| m.name == method) else {
                return Err(ApiError::new(
                    ErrorKind::NotFound,
                    format!("PriyaOS has no request named `{method}`."),
                ));
            };
            if !info.read_only || info.notification.is_some() {
                return Err(ApiError::new(
                    ErrorKind::PermissionDenied,
                    format!(
                        "Modules may only make requests that change nothing; `{method}` isn't one."
                    ),
                ));
            }
            if manifest.grants(&Capability::ApiRead) {
                Ok(())
            } else {
                Err(denied(name, &format!("call `{method}`"), "api.read"))
            }
        }
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

    fn refused(manifest: &ModuleManifest, call: HostCall<'_>) -> ApiError {
        check(manifest, call).unwrap_err()
    }

    // covers: CORE-02
    // suite: unit
    // acceptance: 0.8
    #[test]
    fn capability_check_denies_undeclared_host_call() {
        let bare = manifest("");
        let error = refused(&bare, HostCall::EmitEvent);
        assert_eq!(error.kind, ErrorKind::PermissionDenied);
        assert_eq!(
            error.user_message,
            "The echo module isn't allowed to record events."
        );
        assert!(error.detail.unwrap().contains("events.emit"));
        assert_eq!(
            refused(&bare, HostCall::CallApi("v1.system.status")).kind,
            ErrorKind::PermissionDenied
        );
        let theme = key("ui.theme");
        let error = refused(&bare, HostCall::ReadSetting(&theme));
        assert!(error.detail.unwrap().contains("settings.read:ui"));
        // Its own section needs nothing; another module's is out of reach.
        assert!(check(&bare, HostCall::ReadSetting(&key("modules.echo.prefix"))).is_ok());
        let other = key("modules.weather.city");
        assert_eq!(
            refused(&bare, HostCall::ReadSetting(&other)).kind,
            ErrorKind::PermissionDenied
        );

        // Declared, they are allowed, but only within what they say.
        let granted = manifest("\"events.emit\", \"api.read\", \"settings.read:ui\"");
        assert!(check(&granted, HostCall::EmitEvent).is_ok());
        assert!(check(&granted, HostCall::CallApi("v1.system.status")).is_ok());
        assert!(check(&granted, HostCall::ReadSetting(&theme)).is_ok());
        assert!(check(&granted, HostCall::ReadSetting(&key("ai.strategy"))).is_err());
        // Even with api.read: nothing that changes things, no streams, and
        // only methods that exist.
        for method in ["v1.settings.set", "v1.events.subscribe"] {
            let error = refused(&granted, HostCall::CallApi(method));
            assert_eq!(error.kind, ErrorKind::PermissionDenied, "{method}");
        }
        assert_eq!(
            refused(&granted, HostCall::CallApi("v1.nope")).kind,
            ErrorKind::NotFound
        );
    }
}
