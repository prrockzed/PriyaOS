//! Generates theme files from `ui/design/tokens.json` and checks colour
//! contrast (sub-phase 0.10; 11-design-system-and-ux.md §2). Used by
//! `cargo xtask tokens`, and later by `priya theme apply` (Phase 5).
//!
//! - [`load`]: reads the token file into [`model`] types, which refuse
//!   unknown keys; `ui/design/tokens.schema.json` describes the same shape.
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
mod validate;

use std::path::Path;

pub use error::TokenError;
pub use model::Tokens;
pub use palette::{DropShadow, Palette, Raised};
pub use validate::VERSION;

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

/// The schema the token file follows, for editors and CI. The typed
/// reading below enforces the same rules (unknown keys are refused, and
/// `validate` checks values); a test keeps the two in step.
pub const SCHEMA: &str = include_str!("../../tokens.schema.json");

/// Reads and checks the token file at `path`.
pub fn load(path: &Path) -> Result<Tokens, TokenError> {
    let text = std::fs::read_to_string(path).map_err(|source| TokenError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse(&text)
}

/// Reads token-file text and checks the rules its types don't carry.
pub fn parse(text: &str) -> Result<Tokens, TokenError> {
    let tokens = serde_json::from_str(text)?;
    validate::validate(&tokens)?;
    Ok(tokens)
}
