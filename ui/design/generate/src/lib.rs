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
mod validate;

use std::path::Path;

pub use error::TokenError;
pub use model::Tokens;
pub use palette::{DropShadow, Palette, Raised};
pub use validate::VERSION;

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
