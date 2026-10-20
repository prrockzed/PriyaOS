//! The echo example module: the smallest useful PriyaOS module, written
//! with `priya-module-sdk`. The Python twin in `modules/examples/echo-py`
//! behaves the same way.
//!
//! - `echo.say {text}` → `{text}`, with the `modules.echo.prefix` setting
//!   in front.
//! - `echo.emit {text}` records the event `said {text}` → `{seq}`.
//! - `echo.setting {key}` → `{value}`: reads any setting through priyad,
//!   which shows what a module may and may not read.
//!
//! It runs as a process (`echo-module`, see `module.toml`) or, through
//! [`BUILTIN`], inside priyad.

use priya_module_sdk::{
    ApiError, BuiltinMain, HostClient, InitializeParams, InitializeResult, Module, Value, json,
    no_such_method, params,
};
use serde::Deserialize;

/// The module's name and methods, as in `module.toml`.
pub const NAME: &str = "echo";
pub const METHODS: [&str; 3] = ["echo.say", "echo.emit", "echo.setting"];

/// The echo module.
#[derive(Debug, Default)]
pub struct Echo;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    key: String,
}

impl Module for Echo {
    async fn initialize(
        &self,
        _: &HostClient,
        _: InitializeParams,
    ) -> Result<InitializeResult, ApiError> {
        Ok(InitializeResult::v1(
            NAME,
            env!("CARGO_PKG_VERSION"),
            METHODS,
        ))
    }

    async fn handle(
        &self,
        host: &HostClient,
        method: &str,
        args: Value,
    ) -> Result<Value, ApiError> {
        match method {
            "echo.say" => {
                let Text { text } = params(args)?;
                // Read each time, so a changed setting applies at once.
                let prefix = host.setting("modules.echo.prefix").await?;
                let prefix = prefix.as_str().unwrap_or_default();
                Ok(json!({ "text": format!("{prefix}{text}") }))
            }
            "echo.emit" => {
                let Text { text } = params(args)?;
                let seq = host.emit_event("said", json!({ "text": text })).await?;
                Ok(json!({ "seq": seq }))
            }
            "echo.setting" => {
                let Key { key } = params(args)?;
                Ok(json!({ "value": host.setting(&key).await? }))
            }
            _ => Err(no_such_method(method)),
        }
    }
}

/// Echo as a builtin module, for priyad's builtin table.
pub const BUILTIN: BuiltinMain = priya_module_sdk::builtin::<Echo>;
