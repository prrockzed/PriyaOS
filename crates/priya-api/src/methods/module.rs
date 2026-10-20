//! `v1.module.*`: the modules priyad runs (02-core §5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Done, Method, NoParams};
use crate::module::ModuleKind;

/// `v1.module.list`
pub struct List;

impl Method for List {
    const NAME: &'static str = "v1.module.list";
    const SUMMARY: &'static str = "Lists the modules PriyaOS found and what each is doing.";
    const READ_ONLY: bool = true;
    type Params = NoParams;
    type Result = ListResult;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ListResult {
    /// By name.
    pub modules: Vec<ModuleSummary>,
}

/// One module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModuleSummary {
    pub name: String,
    /// From its module.toml; none when it can't be read.
    pub version: Option<String>,
    pub kind: Option<ModuleKind>,
    pub description: Option<String>,
    /// The methods `v1.module.call` can reach.
    pub methods: Vec<String>,
    /// Whether `modules.enabled` lists it.
    pub enabled: bool,
    pub state: ModuleState,
    /// Its process, while it runs as one.
    pub pid: Option<u32>,
    /// How many times it was restarted after failing.
    pub restarts: u32,
    /// What went wrong last, or why it can't be used.
    pub problem: Option<String>,
}

/// What a module is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModuleState {
    Starting,
    Running,
    /// It failed and will be started again shortly.
    Restarting,
    Stopping,
    Stopped,
    /// It failed too often, or can't work: PriyaOS gave up on it.
    Failed,
    /// Installed but not enabled.
    Disabled,
    /// Its folder is there, but it can't be used.
    Broken,
    /// Enabled, but not installed.
    Missing,
}

/// `v1.module.enable`
pub struct Enable;

impl Method for Enable {
    const NAME: &'static str = "v1.module.enable";
    const SUMMARY: &'static str = "Turns a module on; it starts now and with PriyaOS from then on.";
    const READ_ONLY: bool = false;
    type Params = ModuleName;
    type Result = Done;
}

/// `v1.module.disable`
pub struct Disable;

impl Method for Disable {
    const NAME: &'static str = "v1.module.disable";
    const SUMMARY: &'static str = "Turns a module off; it stops now and stays off.";
    const READ_ONLY: bool = false;
    type Params = ModuleName;
    type Result = Done;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModuleName {
    /// Such as `echo`.
    pub name: String,
}

/// `v1.module.call`
pub struct Call;

impl Method for Call {
    const NAME: &'static str = "v1.module.call";
    const SUMMARY: &'static str = "Calls one of a module's methods and returns its answer.";
    // A module's method may change anything the module may change.
    const READ_ONLY: bool = false;
    type Params = CallParams;
    type Result = Value;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CallParams {
    /// `<module>.<method>`, such as `echo.say`.
    pub method: String,
    /// The method's own params; none means `{}`.
    #[serde(default)]
    pub params: Value,
}
