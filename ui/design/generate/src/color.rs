//! sRGB colours as the token file writes them, and the WCAG 2.2 maths.

use std::fmt;

/// An sRGB colour with alpha, each channel 0–255. The default is fully
/// transparent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Reads `#RRGGBB` or `#RRGGBBAA`.
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Self {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
            a: if hex.len() == 8 { channel(6)? } else { 255 },
        })
    }

    pub fn is_opaque(self) -> bool {
        self.a == 255
    }

    /// This colour drawn over `under`, which should be opaque.
    pub fn over(self, under: Self) -> Self {
        let alpha = f64::from(self.a) / 255.0;
        let mix = |top: u8, bottom: u8| {
            let value = f64::from(top) * alpha + f64::from(bottom) * (1.0 - alpha);
            // In 0–255 by construction.
            value.round().clamp(0.0, 255.0) as u8
        };
        Self {
            r: mix(self.r, under.r),
            g: mix(self.g, under.g),
            b: mix(self.b, under.b),
            a: 255,
        }
    }

    /// WCAG 2.2 relative luminance, 0 (black) to 1 (white).
    pub fn luminance(self) -> f64 {
        let linear = |channel: u8| {
            let c = f64::from(channel) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }
}

/// The WCAG contrast ratio of two opaque colours, 1 to 21.
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (lighter, darker) = if la > lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

impl fmt::Display for Color {
    /// `#RRGGBB`, or `#RRGGBBAA` when translucent.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)?;
        if !self.is_opaque() {
            write!(f, "{:02X}", self.a)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(text: &str) -> Color {
        Color::parse(text).unwrap()
    }

    // covers: DS-06
    // suite: unit
    #[test]
    fn wcag_ratio_matches_reference_values() {
        let ratio = |a, b| contrast_ratio(c(a), c(b));
        assert!((ratio("#000000", "#FFFFFF") - 21.0).abs() < 1e-9);
        assert!(
            (ratio("#FFFFFF", "#000000") - 21.0).abs() < 1e-9,
            "order doesn't matter"
        );
        assert!((ratio("#777777", "#FFFFFF") - 4.48).abs() < 0.01);
        assert!((ratio("#123456", "#123456") - 1.0).abs() < 1e-9);
    }

    // covers: DS-06
    // suite: unit
    #[test]
    fn alpha_colors_composited_before_measuring() {
        let half_black = c("#00000080");
        assert!(!half_black.is_opaque());
        assert_eq!(half_black.over(c("#FFFFFF")), c("#7F7F7F"));
        assert_eq!(
            c("#FF000000").over(c("#00FF00")),
            c("#00FF00"),
            "transparent"
        );
        assert_eq!(half_black.to_string(), "#00000080");
        assert_eq!(c("#abcdef").to_string(), "#ABCDEF");
        for bad in ["ABCDEF", "#ABC", "#GGGGGG", "#1234567"] {
            assert_eq!(Color::parse(bad), None, "{bad}");
        }
    }
}
