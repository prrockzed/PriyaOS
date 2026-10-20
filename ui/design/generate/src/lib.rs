//! Generates theme files from `ui/design/tokens.json` and checks colour
//! contrast (sub-phase 0.10; 11-design-system-and-ux.md §2). Used by
//! `cargo xtask tokens`, and later by `priya theme apply` (Phase 5).
//!
//! - [`load`]: reads the token file into [`model`] types, which refuse
//!   unknown keys; `ui/design/tokens.schema.json` describes the same shape.
//! - [`Palette`]: concrete colours per theme, with inheritance and
//!   references resolved.
//! - [`contrast::check`]: every declared pair against WCAG 2.2.

pub mod color;
pub mod contrast;
mod error;
pub mod model;
mod palette;

use std::path::Path;

pub use error::TokenError;
pub use model::Tokens;
pub use palette::Palette;

/// The schema the token file follows, for editors and CI. The typed
/// reading below enforces the same shape (unknown keys are refused); a test
/// keeps the two in step.
pub const SCHEMA: &str = include_str!("../../tokens.schema.json");

/// Reads and checks the token file at `path`.
pub fn load(path: &Path) -> Result<Tokens, TokenError> {
    let text = std::fs::read_to_string(path).map_err(|source| TokenError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse(&text)
}

/// Reads token-file text.
pub fn parse(text: &str) -> Result<Tokens, TokenError> {
    Ok(serde_json::from_str(text)?)
}
