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

mod diff;
mod edit;
mod error;
mod key;
mod layers;
mod migrate;
mod model;
mod names;
mod strategy;
mod validate;

pub use diff::{Changed, diff};
pub use edit::{Change, ChangeError, EditError, Target, change, set, unset};
pub use error::{Problem, SettingsError, SettingsErrors};
pub use key::KeyPath;
pub use layers::{Content, Layer, Layered, Origin, Source, project_file};
pub use migrate::{MIGRATIONS, MigrateError, Migrated, Migration, migrate};
pub use model::{
    Ai, Bar, CURRENT_VERSION, LogLevel, Logging, Modules, Provider, ProviderKind, Settings, Theme,
    Ui,
};
pub use names::ProviderName;
pub use strategy::{Strategy, StrategyError};
pub use validate::{Loaded, is_setting, load};
