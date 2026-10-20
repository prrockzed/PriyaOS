//! [`FakeModuleProcess`]: a module that misbehaves on purpose, for testing
//! the module host. It is the `priya-fake-module` binary of this crate,
//! told what to do by a `script.json` next to its `module.toml` (priyad
//! starts modules in their own folder).
//!
//! ```ignore
//! FakeModuleProcess::new("flaky").crash_on_call(1).install(&daemon.modules_dir());
//! ```
//!
//! It always answers `initialize` (truthfully unless told to lie) and
//! `shutdown`. Its methods are `<name>.call`, which answers `{"calls": n}`,
//! and `<name>.host {method, params}`, which makes that host call and
//! answers with what priyad said.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::workspace_binary;

/// What the fake module does, as `script.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Script {
    /// Say in the handshake that it has a method its manifest doesn't list.
    pub lie: bool,
    /// Exit with code 3 on the Nth call of one of its methods.
    pub crash_on_call: Option<u32>,
    /// After the handshake, answer nothing more (pings included).
    pub hang: bool,
    /// After the handshake, write something that isn't a message.
    pub garbage: bool,
    /// After the handshake, announce a message over 16 MiB.
    pub oversize: bool,
}

/// A fake module to install in a modules folder.
#[derive(Debug, Clone)]
pub struct FakeModuleProcess {
    name: String,
    script: Script,
    capabilities: Vec<String>,
    settings_schema: Option<String>,
}

impl FakeModuleProcess {
    /// A well-behaved fake module called `name`.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            script: Script::default(),
            capabilities: Vec::new(),
            settings_schema: None,
        }
    }

    pub fn crash_on_call(mut self, n: u32) -> Self {
        self.script.crash_on_call = Some(n);
        self
    }

    pub fn hang(mut self) -> Self {
        self.script.hang = true;
        self
    }

    pub fn garbage(mut self) -> Self {
        self.script.garbage = true;
        self
    }

    pub fn oversize(mut self) -> Self {
        self.script.oversize = true;
        self
    }

    pub fn lie(mut self) -> Self {
        self.script.lie = true;
        self
    }

    /// Asks for a capability in its manifest.
    pub fn capability(mut self, capability: &str) -> Self {
        self.capabilities.push(capability.to_owned());
        self
    }

    /// Gives it a settings schema (JSON text).
    pub fn settings_schema(mut self, schema: &str) -> Self {
        self.settings_schema = Some(schema.to_owned());
        self
    }

    /// Writes `<modules>/<name>/` with its manifest and script; returns the
    /// folder. The binary must be built (`cargo build -p priya-testkit`).
    pub fn install(&self, modules: &Path) -> PathBuf {
        let dir = modules.join(&self.name);
        fs::create_dir_all(&dir).expect("the module's folder");
        let name = &self.name;
        let exec = workspace_binary("priya-fake-module");
        let capabilities: Vec<String> = self
            .capabilities
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect();
        let schema = if self.settings_schema.is_some() {
            "settings_schema = \"settings.schema.json\"\n"
        } else {
            ""
        };
        let manifest = format!(
            "[module]\nname = \"{name}\"\nversion = \"0.1.0\"\napi = \"v1\"\nkind = \"process\"\n\
             exec = \"{}\"\n[provides]\nmethods = [\"{name}.call\", \"{name}.host\"]\n{schema}\
             [requires]\ncapabilities = [{}]\n",
            exec.display(),
            capabilities.join(", ")
        );
        fs::write(dir.join("module.toml"), manifest).expect("the manifest");
        let script = serde_json::to_string(&self.script).expect("a script");
        fs::write(dir.join("script.json"), script).expect("the script");
        if let Some(schema) = &self.settings_schema {
            fs::write(dir.join("settings.schema.json"), schema).expect("the schema");
        }
        dir
    }
}
