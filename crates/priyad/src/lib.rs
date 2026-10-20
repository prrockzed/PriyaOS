//! priyad, the per-user PriyaOS daemon. Its API and module host arrive in
//! sub-phases 0.7 and 0.8; so far it has the API, doctor and the settings
//! service. Logging is in `priya-logging`, shared with the CLI.

pub mod api;
pub mod daemon;
pub mod doctor;
pub mod rpc;
pub mod settings;
