//! Settings (sub-phase 0.6, CORE-03; design in 02-core §6).
//!
//! [`Settings`] is the typed, merged result of every settings layer. Its
//! defaults live in code ([`Settings::default`]) and are mirrored, key for
//! key, in `data/defaults/00-core.toml`, which ships as
//! `/usr/share/priya/defaults/00-core.toml`; a test keeps the two in step.
//!
//! Core sections refuse unknown keys. Module sections (`modules.<name>`) are
//! kept as plain JSON here and checked against each module's own schema by
//! the module host (0.8).

mod model;
mod names;
mod strategy;

pub use model::{
    Ai, Bar, CURRENT_VERSION, LogLevel, Logging, Modules, Provider, ProviderKind, Settings, Theme,
    Ui,
};
pub use names::ProviderName;
pub use strategy::{Strategy, StrategyError};
