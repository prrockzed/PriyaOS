//! The contrast check (DS-06): every declared pair, in every checked
//! theme, against the WCAG 2.2 threshold for its kind.

use std::fmt::Write as _;

use crate::color::{Color, contrast_ratio};
use crate::error::TokenError;
use crate::model::{ContrastLevel, Kind, REQUIRED_THEMES, Tokens};
use crate::palette::Palette;

/// One foreground on one background in one theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    pub theme: String,
    pub fg: String,
    pub bg: String,
    pub kind: Kind,
    pub ratio: f64,
    pub needed: f64,
}

impl Measured {
    pub fn passes(&self) -> bool {
        self.ratio >= self.needed
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub measured: Vec<Measured>,
    /// Text colours that no pair checks, so nobody knows they're readable.
    pub unchecked_text: Vec<String>,
}

impl Report {
    pub fn passes(&self) -> bool {
        self.unchecked_text.is_empty() && self.measured.iter().all(Measured::passes)
    }

    /// One line per problem, for an error message.
    pub fn problems(&self) -> Vec<String> {
        let mut problems: Vec<String> = self
            .measured
            .iter()
            .filter(|m| !m.passes())
            .map(|m| {
                format!(
                    "{} on {} in {}: {:.2}:1, needs {}:1",
                    m.fg, m.bg, m.theme, m.ratio, m.needed
                )
            })
            .collect();
        problems.extend(
            self.unchecked_text
                .iter()
                .map(|token| format!("{token} is text but no contrast pair checks it")),
        );
        problems
    }

    /// The report committed for people: a table per theme.
    pub fn to_markdown(&self) -> String {
        let mut out = String::from(
            "# Contrast report\n\nGenerated from ui/design/tokens.json — do not edit.\n",
        );
        let mut theme = None;
        for m in &self.measured {
            if theme != Some(&m.theme) {
                theme = Some(&m.theme);
                let _ = write!(
                    out,
                    "\n## {}\n\n| Foreground | Background | Kind | Ratio | Needed | |\n|---|---|---|---|---|---|\n",
                    m.theme
                );
            }
            let kind = match m.kind {
                Kind::Text => "text",
                Kind::LargeText => "large text",
                Kind::NonText => "non-text",
            };
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {kind} | {:.2}:1 | {}:1 | {} |",
                m.fg,
                m.bg,
                m.ratio,
                m.needed,
                if m.passes() { "✓" } else { "✗" }
            );
        }
        for token in &self.unchecked_text {
            let _ = writeln!(out, "\n✗ `{token}` is text but no pair checks it.");
        }
        out
    }
}

/// The ratio `kind` needs at a contrast `level` (WCAG 2.2: AA 1.4.3 and
/// 1.4.11; AAA 1.4.6 for high contrast).
pub fn needed(kind: Kind, level: ContrastLevel) -> f64 {
    match (kind, level) {
        (Kind::NonText, _) => 3.0,
        (Kind::Text, ContrastLevel::Normal) => 4.5,
        (Kind::LargeText, ContrastLevel::Normal) => 3.0,
        (Kind::Text, ContrastLevel::High) => 7.0,
        (Kind::LargeText, ContrastLevel::High) => 4.5,
    }
}

/// Measures every declared pair in light, dark and every other theme
/// marked `checked`.
pub fn check(tokens: &Tokens, palette: &Palette) -> Result<Report, TokenError> {
    let mut measured = Vec::new();
    for (theme, info) in &tokens.themes {
        if !REQUIRED_THEMES.contains(&theme.as_str()) && info.checked != Some(true) {
            continue;
        }
        let color = |name: &str| {
            palette
                .get(theme, name)
                .ok_or_else(|| TokenError::UnknownPairColor {
                    name: name.to_owned(),
                    theme: theme.clone(),
                })
        };
        let base = color("bg.base")?;
        if !base.is_opaque() {
            return Err(TokenError::TranslucentBase(theme.clone()));
        }
        for pair in &tokens.contrast {
            for bg_name in &pair.bg {
                // Translucent colours are measured as drawn: a background
                // over the page, a foreground over its background.
                let bg = opaque(color(bg_name)?, base);
                let fg = opaque(color(&pair.fg)?, bg);
                measured.push(Measured {
                    theme: theme.clone(),
                    fg: pair.fg.clone(),
                    bg: bg_name.clone(),
                    kind: pair.kind,
                    ratio: contrast_ratio(fg, bg),
                    needed: needed(pair.kind, info.contrast),
                });
            }
        }
    }
    // A text colour counts as checked only by a pair that checks it as text.
    let unchecked_text = palette
        .names()
        .into_iter()
        .filter(|name| name.starts_with("text.") || name.starts_with("on."))
        .filter(|name| {
            !tokens
                .contrast
                .iter()
                .any(|pair| pair.fg == *name && pair.kind.is_text())
        })
        .map(str::to_owned)
        .collect();
    Ok(Report {
        measured,
        unchecked_text,
    })
}

fn opaque(color: Color, under: Color) -> Color {
    if color.is_opaque() {
        color
    } else {
        color.over(under)
    }
}
