//! Settings migrations, keyed on each file's `settings_version` (0.6; the
//! synthetic cases are exercised in 0.9).
//!
//! A migration rewrites one file's document from one version to the next,
//! with `toml_edit`, so the owner's comments survive. A file without
//! `settings_version` is version 1, the first format. A file newer than this
//! build is refused rather than misread.

use priya_api::error::{ErrorKind, UserFacing};
use toml_edit::DocumentMut;

use super::model::CURRENT_VERSION;

/// One step: turns a version `from` document into a version `from + 1` one.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub from: u32,
    pub apply: fn(&mut DocumentMut),
}

/// Every step, oldest first. Empty until the format first changes.
pub const MIGRATIONS: &[Migration] = &[];

/// The result of [`migrate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migrated {
    /// Already current; use the text as it is.
    Current,
    /// Rewritten from version `from` to the current one.
    Upgraded { text: String, from: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MigrateError {
    #[error("this file isn't valid TOML: {0}")]
    Syntax(String),
    #[error("`settings_version` must be a whole number of 1 or more")]
    BadVersion,
    #[error(
        "this file is for a newer version of PriyaOS (format {found}; this one reads up to {supported}); update PriyaOS or remove the newer settings"
    )]
    Newer { found: u32, supported: u32 },
    /// A gap in [`MIGRATIONS`]: a bug, caught by a test.
    #[error("PriyaOS can't update settings from format {0}")]
    NoStep(u32),
}

impl UserFacing for MigrateError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::NoStep(_) => ErrorKind::Internal,
            _ => ErrorKind::InvalidInput,
        }
    }

    fn user_message(&self) -> String {
        let message = self.to_string();
        let mut chars = message.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    }
}

/// Brings one settings file's text up to [`CURRENT_VERSION`].
pub fn migrate(text: &str) -> Result<Migrated, MigrateError> {
    migrate_with(text, MIGRATIONS, CURRENT_VERSION)
}

fn migrate_with(
    text: &str,
    migrations: &[Migration],
    current: u32,
) -> Result<Migrated, MigrateError> {
    let mut document: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| MigrateError::Syntax(e.message().to_owned()))?;
    let found = match document.get("settings_version") {
        None => 1,
        Some(item) => item
            .as_integer()
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v >= 1)
            .ok_or(MigrateError::BadVersion)?,
    };
    if found > current {
        return Err(MigrateError::Newer {
            found,
            supported: current,
        });
    }
    if found == current {
        return Ok(Migrated::Current);
    }
    for version in found..current {
        let step = migrations
            .iter()
            .find(|m| m.from == version)
            .ok_or(MigrateError::NoStep(version))?;
        (step.apply)(&mut document);
    }
    document["settings_version"] = toml_edit::value(i64::from(current));
    Ok(Migrated::Upgraded {
        text: document.to_string(),
        from: found,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-08, CORE-03
    // suite: unit
    // acceptance: 0.9
    #[test]
    fn old_settings_fixture_migrates_to_current() {
        use crate::settings::{Layer, Source, Theme, load};
        // A synthetic format change: the next format renames `ui.colours`
        // to `ui.theme` and needs `ui.bar.width_px` set. The old file is
        // what `cargo xtask schemas --check` wants under
        // tests/fixtures/settings/v<N>/ when the format changes.
        fn step(document: &mut DocumentMut) {
            if let Some(ui) = document.get_mut("ui").and_then(|t| t.as_table_mut())
                && let Some(old) = ui.remove("colours")
            {
                ui.insert("theme", old);
            }
            document["ui"]["bar"]["width_px"] = toml_edit::value(720);
        }
        let old = "settings_version = 1\n# mine\n[ui]\ncolours = \"light\"\n";
        let steps = [Migration {
            from: 1,
            apply: step,
        }];
        let Migrated::Upgraded { text, from: 1 } = migrate_with(old, &steps, 2).unwrap() else {
            panic!("expected an upgrade from 1");
        };
        // The result is a valid file of today's settings (whose format is
        // still 1, so the synthetic version number is left out).
        let mut document: DocumentMut = text.parse().unwrap();
        assert_eq!(document["settings_version"].as_integer(), Some(2));
        document.remove("settings_version");
        let loaded = load(
            [Source::toml(Layer::User, "old.toml", document.to_string())],
            None,
        )
        .expect("valid settings");
        assert_eq!(loaded.settings.ui.theme, Theme::Light);
        assert_eq!(loaded.settings.ui.bar.width_px, 720);
        // Without the step, the old file isn't valid any more.
        assert!(load([Source::toml(Layer::User, "old.toml", old)], None).is_err());
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn migrations_run_in_order_and_keep_comments() {
        fn rename_theme(document: &mut DocumentMut) {
            if let Some(ui) = document.get_mut("ui").and_then(|i| i.as_table_mut())
                && let Some(old) = ui.remove("colours")
            {
                ui.insert("theme", old);
            }
        }
        fn add_bar(document: &mut DocumentMut) {
            document["ui"]["bar"]["width_px"] = toml_edit::value(720);
        }
        let steps = [
            Migration {
                from: 2,
                apply: add_bar,
            },
            Migration {
                from: 1,
                apply: rename_theme,
            },
        ];
        let text = "# mine\n[ui]\ncolours = \"dark\" # at night\n";
        let Migrated::Upgraded { text, from } = migrate_with(text, &steps, 3).unwrap() else {
            panic!("expected an upgrade");
        };
        assert_eq!(from, 1, "no version means the first format");
        // The comment stays with its table; the new version line goes first.
        assert!(
            text.starts_with("settings_version = 3\n# mine\n[ui]\n"),
            "{text}"
        );
        assert!(text.contains("theme = \"dark\" # at night"), "{text}");
        assert!(text.contains("width_px = 720"), "{text}");

        assert_eq!(
            migrate_with("settings_version = 3\n", &steps, 3),
            Ok(Migrated::Current)
        );
        assert_eq!(
            migrate_with("", &steps[..1], 3),
            Err(MigrateError::NoStep(1))
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn newer_or_broken_versions_are_refused_plainly() {
        let err = migrate_with("settings_version = 9\n", &[], 1).unwrap_err();
        assert_eq!(
            err,
            MigrateError::Newer {
                found: 9,
                supported: 1
            }
        );
        assert!(
            err.user_message()
                .starts_with("This file is for a newer version of PriyaOS")
        );
        for bad in [
            "settings_version = 0\n",
            "settings_version = \"1\"\n",
            "settings_version = -2\n",
        ] {
            assert_eq!(
                migrate_with(bad, &[], 1),
                Err(MigrateError::BadVersion),
                "{bad}"
            );
        }
        assert_eq!(migrate("settings_version = 1\n"), Ok(Migrated::Current));
        assert_eq!(migrate("[ui]\n"), Ok(Migrated::Current));
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    #[allow(
        clippy::reversed_empty_ranges,
        reason = "empty until the settings format first changes"
    )]
    fn shipped_migrations_have_no_gaps() {
        for version in 1..CURRENT_VERSION {
            assert!(
                MIGRATIONS.iter().any(|m| m.from == version),
                "no migration from settings format {version}"
            );
        }
        assert!(MIGRATIONS.iter().all(|m| m.from < CURRENT_VERSION));
    }
}
