//! Modules (02-core §5): the `module.toml` manifest and the protocol
//! between priyad and a module.

pub mod manifest;
pub mod protocol;

pub use manifest::{
    API_VERSION, Capability, DefaultMode, ManifestError, ModuleInfo, ModuleKind, ModuleManifest,
    Provides, Requires, Subscribes, Surface, SurfaceKind,
};
