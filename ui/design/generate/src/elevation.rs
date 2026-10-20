//! Elevation flattened for generators: every theme gets the same named
//! shadow layers and borders, transparent where it has none, so outputs
//! can switch themes without missing values.

use std::collections::BTreeMap;

use crate::color::Color;
use crate::names::camel;
use crate::palette::Palette;

/// One shadow layer, named for QML (`elevationCardShadow1`).
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// The elevation level it belongs to (`card`).
    pub level: String,
    pub name: String,
    pub color: Color,
    pub y: f64,
    pub blur: f64,
}

/// The most shadow layers any theme has, per level, and whether any theme
/// has a border, per level.
fn shape(palette: &Palette) -> BTreeMap<&str, (usize, bool)> {
    let mut shape: BTreeMap<&str, (usize, bool)> = BTreeMap::new();
    for levels in palette.elevation.values() {
        for (level, raised) in levels {
            let entry = shape.entry(level.as_str()).or_default();
            entry.0 = entry.0.max(raised.shadows.len());
            entry.1 |= raised.border.is_some();
        }
    }
    shape
}

/// `theme`'s shadow layers, numbered from 1 in drawing order.
pub fn layers(palette: &Palette, theme: &str) -> Vec<Layer> {
    let mut out = Vec::new();
    for (level, (count, _)) in shape(palette) {
        let raised = palette.elevation.get(theme).and_then(|l| l.get(level));
        for i in 0..count {
            let shadow = raised.and_then(|r| r.shadows.get(i));
            out.push(Layer {
                level: level.to_owned(),
                name: camel(&format!("elevation.{level}.shadow{}", i + 1)),
                color: shadow.map(|s| s.color).unwrap_or_default(),
                y: shadow.map_or(0.0, |s| s.y),
                blur: shadow.map_or(0.0, |s| s.blur),
            });
        }
    }
    out
}

/// `theme`'s elevation borders by name (`elevationCardBorder`).
pub fn borders(palette: &Palette, theme: &str) -> Vec<(String, Color)> {
    shape(palette)
        .into_iter()
        .filter(|(_, (_, has_border))| *has_border)
        .map(|(level, _)| {
            let border = palette
                .elevation
                .get(theme)
                .and_then(|l| l.get(level))
                .and_then(|r| r.border);
            (
                camel(&format!("elevation.{level}.border")),
                border.unwrap_or_default(),
            )
        })
        .collect()
}
