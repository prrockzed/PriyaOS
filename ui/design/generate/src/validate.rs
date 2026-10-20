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
    themes(tokens, &mut problems);
    names(tokens, &mut problems);
    sizes(tokens, &mut problems);
    elevation(tokens, &mut problems);
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

fn themes(tokens: &Tokens, problems: &mut Vec<String>) {
    for theme in REQUIRED_THEMES {
        if tokens
            .themes
            .get(theme)
            .is_some_and(|t| t.checked.is_some())
        {
            problems.push(format!("{theme} is always checked; leave out `checked`"));
        }
    }
    for (theme, info) in &tokens.themes {
        if info.name.trim().is_empty() {
            problems.push(format!("theme `{theme}` needs a name"));
        }
    }
}

/// Names become QML properties and CSS selectors, so they stay simple, and
/// don't hide the palette's own.
fn names(tokens: &Tokens, problems: &mut Vec<String>) {
    let mut plain = |what: &str, name: &str, dashes: bool| {
        if !is_plain(name, dashes) {
            let allowed = if dashes {
                "lowercase letters, digits and -"
            } else {
                "lowercase letters and digits"
            };
            problems.push(format!("{what} `{name}`: use {allowed}"));
        }
    };
    for theme in tokens.themes.keys() {
        plain("theme", theme, true);
    }
    for option in tokens.accent.options.keys() {
        plain("accent", option, true);
    }
    for level in tokens.elevation.keys() {
        plain("elevation", level, false);
    }
    for name in tokens.color.keys() {
        if !name.split('.').all(|part| is_plain(part, false)) {
            problems.push(format!(
                "`color.{name}`: use lowercase letters and digits, in parts joined by dots"
            ));
        }
        if name == "focus" || name.starts_with("mode.") || ACCENT_ROLES.contains(&name.as_str()) {
            problems.push(format!(
                "`color.{name}` would hide the palette's own `{name}`; pick another name"
            ));
        }
    }
    let fonts = &tokens.font;
    for (kind, font) in [
        ("ui", &fonts.ui),
        ("mono", &fonts.mono),
        ("indic", &fonts.indic),
    ] {
        if font.trim().is_empty() {
            problems.push(format!("font.{kind} needs a font name"));
        }
    }
}

fn sizes(tokens: &Tokens, problems: &mut Vec<String>) {
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
}

fn elevation(tokens: &Tokens, problems: &mut Vec<String>) {
    for (level, themes) in &tokens.elevation {
        for (theme, elevation) in themes {
            let at = format!("elevation.{level}.{theme}");
            match &elevation.shadow {
                Some(layers) if layers.is_empty() => {
                    problems.push(format!("{at}: a shadow needs at least one layer"));
                }
                None if elevation.border.is_none() => {
                    problems.push(format!("{at} needs a shadow or a border"));
                }
                _ => {}
            }
            if elevation.shadow.iter().flatten().any(|s| s.blur < 0.0) {
                problems.push(format!("{at}: blur can't be negative"));
            }
        }
    }
}

/// `[a-z][a-z0-9]*`, with `-` allowed after the first letter when
/// `dashes`.
fn is_plain(name: &str, dashes: bool) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || (dashes && c == '-'))
}

/// Whether a pair with `kind` checks a foreground as text.
pub fn is_text(kind: Kind) -> bool {
    matches!(kind, Kind::Text | Kind::LargeText)
}
