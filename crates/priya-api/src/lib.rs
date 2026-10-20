//! Wire types shared by priyad, its clients, modules and priya-helper.
//!
//! Everything here is part of the public, versioned API and has a JSON
//! Schema. This crate depends on nothing else in the workspace.

#[cfg(feature = "client")]
pub mod client;
pub mod error;
pub mod events;
pub mod framing;
pub mod methods;
pub mod module;
#[cfg(feature = "peer")]
pub mod peer;
pub mod rpc;
