//! The token file as Rust types (`ui/design/tokens.schema.json` says the
//! same for editors and CI). Colour values stay text here: `#RRGGBB`,
//! `#RRGGBBAA` or a reference such as `{color.info}`, resolved per theme
//! by [`crate::palette`].

use std::collections::BTreeMap;

use serde::Deserialize;

/// A value per theme: `light` and `dark` always, others when they differ
/// from the theme they inherit from.
pub type PerTheme<T = String> = BTreeMap<String, T>;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tokens {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    pub version: u32,
    pub themes: BTreeMap<String, ThemeInfo>,
    pub color: BTreeMap<String, PerTheme>,
    pub accent: Accents,
    pub mode: Modes,
    pub font: Fonts,
    #[serde(rename = "type")]
    pub type_scale: TypeScale,
    pub space: Vec<u32>,
    pub radius: Radii,
    pub motion: Motion,
    pub elevation: BTreeMap<String, PerTheme<Elevation>>,
    pub focus: Focus,
    pub contrast: Vec<Pair>,
}

/// The themes every token file has; their contrast is always checked.
pub const REQUIRED_THEMES: [&str; 2] = ["light", "dark"];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeInfo {
    pub name: String,
    /// Where values this theme doesn't give come from.
    #[serde(default)]
    pub inherits: Option<String>,
    /// Whether CI checks an optional theme's contrast (from sub-phase 5.11);
    /// light and dark are always checked.
    #[serde(default)]
    pub checked: Option<bool>,
    /// How much contrast it promises.
    #[serde(default)]
    pub contrast: ContrastLevel,
}

/// WCAG AA, or AAA for the high-contrast themes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContrastLevel {
    #[default]
    Normal,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accents {
    /// The accent that gives the palette's `accent`, `accent.subtle` and
    /// `on.accent` (referable as `{color.accent}` and so on).
    pub default: String,
    pub options: BTreeMap<String, Accent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accent {
    pub name: String,
    pub accent: PerTheme,
    #[serde(rename = "accent.subtle")]
    pub subtle: PerTheme,
    #[serde(rename = "on.accent")]
    pub on_accent: PerTheme,
}

/// The colours each accent gives, by the palette names they take.
pub const ACCENT_ROLES: [&str; 3] = ["accent", "accent.subtle", "on.accent"];

impl Accent {
    /// Its colours by role, as references name them (`accent.subtle`).
    pub fn roles(&self) -> [(&'static str, &PerTheme); 3] {
        [
            ("accent", &self.accent),
            ("accent.subtle", &self.subtle),
            ("on.accent", &self.on_accent),
        ]
    }
}

/// Each work mode's colour, always with the same meaning (11 §2.1).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modes {
    pub look: String,
    pub sandbox: String,
    pub direct: String,
    pub direct_raw: String,
}

impl Modes {
    pub fn all(&self) -> [(&'static str, &str); 4] {
        [
            ("look", &self.look),
            ("sandbox", &self.sandbox),
            ("direct", &self.direct),
            ("direct_raw", &self.direct_raw),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    pub ui: String,
    pub mono: String,
    pub indic: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeScale {
    pub caption: TypeStyle,
    pub small: TypeStyle,
    pub body: TypeStyle,
    #[serde(rename = "bodyLarge")]
    pub body_large: TypeStyle,
    pub title: TypeStyle,
    pub headline: TypeStyle,
    pub display: TypeStyle,
}

impl TypeScale {
    /// Every style with its token name, smallest first.
    pub fn all(&self) -> [(&'static str, &TypeStyle); 7] {
        [
            ("caption", &self.caption),
            ("small", &self.small),
            ("body", &self.body),
            ("bodyLarge", &self.body_large),
            ("title", &self.title),
            ("headline", &self.headline),
            ("display", &self.display),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TypeStyle {
    /// In pixels at scale 1.0.
    pub size: f64,
    pub weight: u16,
    pub line_height: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Radii {
    pub control: Radius,
    pub card: Radius,
    pub panel: Radius,
    pub pill: Radius,
}

impl Radii {
    pub fn all(&self) -> [(&'static str, &Radius); 4] {
        [
            ("control", &self.control),
            ("card", &self.card),
            ("panel", &self.panel),
            ("pill", &self.pill),
        ]
    }
}

/// A corner radius in pixels, or fully round.
#[derive(Debug, Clone, PartialEq)]
pub enum Radius {
    Pixels(f64),
    Full,
}

impl<'de> Deserialize<'de> for Radius {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::Number(n) if n.as_f64().is_some_and(|px| px >= 0.0) => {
                Ok(Self::Pixels(n.as_f64().unwrap_or_default()))
            }
            serde_json::Value::String(s) if s == "full" => Ok(Self::Full),
            other => Err(serde::de::Error::custom(format!(
                "a radius is pixels (0 or more) or \"full\", not {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    /// Durations in milliseconds.
    pub micro: u32,
    pub standard: u32,
    pub panel: u32,
    pub easing: Easings,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Easings {
    /// Cubic-bézier control points `[x1, y1, x2, y2]`.
    pub standard: [f64; 4],
    pub emphasised: [f64; 4],
}

/// How a raised surface stands out: shadows (light) or a border (dark).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Elevation {
    /// Layers, drawn together (CSS `box-shadow` order).
    #[serde(default)]
    pub shadow: Vec<Shadow>,
    #[serde(default)]
    pub border: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shadow {
    pub y: f64,
    pub blur: f64,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Focus {
    pub width: f64,
    pub color: String,
}

/// A foreground that must stay readable on each of its backgrounds.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// Palette names: colour tokens (`text.primary`), `focus` or
    /// `mode.<name>`.
    pub fg: String,
    pub bg: Vec<String>,
    pub kind: Kind,
}

/// What the foreground is, which sets the ratio it needs (WCAG 2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Body text: 4.5:1 (1.4.3).
    Text,
    /// At least 24 px, or 18.66 px at weight 600: 3:1 (1.4.3).
    LargeText,
    /// Input outlines, focus rings, meaningful icons: 3:1 (1.4.11).
    NonText,
}
