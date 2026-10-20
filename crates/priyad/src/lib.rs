//! priyad, the per-user PriyaOS daemon: the API, doctor, the settings
//! service and, from 0.8, the module host. Logging is in `priya-logging`, shared with the CLI.

pub mod ai;
pub mod api;
pub mod daemon;
pub mod doctor;
pub mod module_api;
pub mod modules;
pub mod rpc;
pub mod settings;
