//! The rules of `tokens.schema.json` that the types alone don't enforce,
//! so `cargo xtask tokens` and `priya theme apply` refuse what CI's schema
//! check would.

use crate::error::TokenError;
use crate::model::{ACCENT_ROLES, Kind, REQUIRED_THEMES, Tokens};

/// The only version this crate reads.
pub const VERSION: u32 = 1;

/// Every rule broken, one line each, or none.
pub fn validate(tokens: &Tokens) -> Result<(), TokenError> {
    let mut problems = Vec::new();
    if tokens.version != VERSION {
        problems.push(format!(
            "version is {}, but only {VERSION} can be read",
            tokens.version
        ));
    }
    for theme in REQUIRED_THEMES {
        if tokens
            .themes
            .get(theme)
            .is_some_and(|t| t.checked == Some(false))
        {
            problems.push(format!("{theme} is always checked; leave out `checked`"));
        }
    }
    for name in tokens.color.keys() {
        if name == "focus" || name.starts_with("mode.") || ACCENT_ROLES.contains(&name.as_str()) {
            problems.push(format!(
                "`color.{name}` would hide the palette's own `{name}`; pick another name"
            ));
        }
    }
    for (name, style) in tokens.type_scale.all() {
        if style.size <= 0.0 {
            problems.push(format!("type.{name}.size must be more than 0"));
        }
        if ![400, 500, 600].contains(&style.weight) {
            problems.push(format!("type.{name}.weight must be 400, 500 or 600"));
        }
        if style.line_height < 1.0 {
            problems.push(format!("type.{name}.lineHeight must be at least 1"));
        }
    }
    if tokens.space.is_empty() {
        problems.push("space needs at least one step".to_owned());
    }
    if tokens.focus.width <= 0.0 {
        problems.push("focus.width must be more than 0".to_owned());
    }
    for (level, themes) in &tokens.elevation {
        for (theme, elevation) in themes {
            if elevation.shadow.is_empty() && elevation.border.is_none() {
                problems.push(format!(
                    "elevation.{level}.{theme} needs a shadow or a border"
                ));
            }
            if elevation.shadow.iter().any(|s| s.blur < 0.0) {
                problems.push(format!("elevation.{level}.{theme}: blur can't be negative"));
            }
        }
    }
    for (i, pair) in tokens.contrast.iter().enumerate() {
        if pair.bg.is_empty() {
            problems.push(format!(
                "contrast pair {} ({}) has no background to check against",
                i + 1,
                pair.fg
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(TokenError::Invalid(problems))
    }
}

/// Whether a pair with `kind` checks a foreground as text.
pub fn is_text(kind: Kind) -> bool {
    matches!(kind, Kind::Text | Kind::LargeText)
}
