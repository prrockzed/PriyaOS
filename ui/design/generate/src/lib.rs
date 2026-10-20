//! Generates theme files from `ui/design/tokens.json` and checks colour
//! contrast (sub-phase 0.10; 11-design-system-and-ux.md §2). Used by
//! `cargo xtask tokens`, and later by `priya theme apply` (Phase 5).
//!
//! - [`load`]: checks the token file against `ui/design/tokens.schema.json`,
//!   then reads it into [`model`] types.
//! - [`Palette`]: concrete colours per theme, with inheritance and
//!   references resolved.
//! - [`contrast::check`]: every declared pair against WCAG 2.2.
//! - [`generate`]: `Theme.qml`, `tokens.css` and the contrast report.

pub mod color;
pub mod contrast;
mod css;
mod elevation;
mod error;
pub mod model;
mod names;
mod palette;
mod qml;

use std::path::Path;

pub use error::TokenError;
pub use model::Tokens;
pub use palette::{DropShadow, Palette, Raised};

/// The first line of every generated file, in its comment syntax.
pub const GENERATED: &str =
    "Generated from ui/design/tokens.json by `cargo xtask tokens`; do not edit.";

/// The generated files, as text.
#[derive(Debug, Clone, PartialEq)]
pub struct Outputs {
    /// For `ui/PriyaUI/Theme.qml`.
    pub theme_qml: String,
    /// For `ui/design/build/tokens.css`.
    pub tokens_css: String,
    /// The contrast check, with its report for `target/design/contrast.md`.
    pub contrast: contrast::Report,
}

/// Resolves, checks and generates everything from the tokens. Contrast
/// failures are in the report, not an error, so the report can show them.
pub fn generate(tokens: &Tokens) -> Result<Outputs, TokenError> {
    let palette = Palette::resolve(tokens)?;
    Ok(Outputs {
        theme_qml: qml::theme_qml(tokens, &palette),
        tokens_css: css::tokens_css(tokens, &palette),
        contrast: contrast::check(tokens, &palette)?,
    })
}

/// The schema the token file follows: `load` and `parse` enforce it, and
/// editors can use it too.
pub const SCHEMA: &str = include_str!("../../tokens.schema.json");

/// Reads and checks the token file at `path`.
pub fn load(path: &Path) -> Result<Tokens, TokenError> {
    let text = std::fs::read_to_string(path).map_err(|source| TokenError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse(&text)
}

/// Reads token-file text: checked against the schema first, so every
/// rule lives in one place, then read into the [`model`] types.
pub fn parse(text: &str) -> Result<Tokens, TokenError> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let problems: Vec<String> = schema()
        .iter_errors(&value)
        .map(|e| {
            let at = e.instance_path().to_string();
            if at.is_empty() {
                e.to_string()
            } else {
                format!("at {at}: {e}")
            }
        })
        .collect();
    if !problems.is_empty() {
        return Err(TokenError::Schema(problems));
    }
    Ok(serde_json::from_value(value)?)
}

/// The compiled schema, built once.
fn schema() -> &'static jsonschema::Validator {
    static VALIDATOR: std::sync::LazyLock<jsonschema::Validator> = std::sync::LazyLock::new(|| {
        let schema = serde_json::from_str(SCHEMA).expect("tokens.schema.json is JSON");
        jsonschema::validator_for(&schema).expect("tokens.schema.json is a valid schema")
    });
    &VALIDATOR
}
