//! The seed of Priya AI's gateway (04-priya-ai §1): which model answers,
//! what it is told, and the provider that reaches it. Phase 0 has one
//! model at a time; the router, catalog and optimizer come in Phase 1.

pub mod route;

pub use route::{Route, RouteError, route};
