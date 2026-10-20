//! Settings (sub-phase 0.6, CORE-03; design in 02-core §6).
//!
//! [`Settings`] is the typed, merged result of every settings layer. Its
//! defaults live in code ([`Settings::default`]) and are mirrored, key for
//! key, in `data/defaults/00-core.toml`, which ships as
//! `/usr/share/priya/defaults/00-core.toml`; a test keeps the two in step.
//!
//! [`load`] merges the layers ([`layers`] explains the order) and checks
//! the result, with plain messages that name the file, line and key.
//!
//! Core sections refuse unknown keys. Module sections (`modules.<name>`) are
//! kept as plain JSON here and checked against each module's own schema by
//! the module host (0.8).

mod error;
mod layers;
mod model;
mod names;
mod strategy;
mod validate;

pub use error::{Problem, SettingsError, SettingsErrors};
pub use layers::{Content, Layer, Layered, Origin, Source, project_file};
pub use model::{
    Ai, Bar, CURRENT_VERSION, LogLevel, Logging, Modules, Provider, ProviderKind, Settings, Theme,
    Ui,
};
pub use names::ProviderName;
pub use strategy::{Strategy, StrategyError};
pub use validate::{Loaded, load};
