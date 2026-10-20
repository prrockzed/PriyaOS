//! The settings service (sub-phase 0.6, CORE-03; design in 02-core §6).
//!
//! [`SettingsService`] reads the layer folders with
//! `priya_core::settings::load`, keeps the last valid settings, records a
//! `setting.changed` per changed value and a `settings.load_failed` per
//! broken file version, and changes the user's file with `set`/`unset`,
//! checking the result before an atomic write. [`watch`] reloads when the
//! folders change. Consumers read and watch through a [`SettingsHandle`].

mod files;
mod service;
mod watcher;

pub use files::{Read, SettingsDirs, is_settings_file, read, write_atomic};
pub use service::{EventSink, PrefixWatch, SetError, SettingsHandle, SettingsService};
pub use watcher::{DEBOUNCE, SettingsWatcher, watch};
