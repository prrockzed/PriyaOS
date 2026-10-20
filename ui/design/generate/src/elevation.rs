//! Elevation flattened for generators: every theme gets the same named
//! shadow layers and borders, transparent where it has none, so outputs
//! can switch themes without missing values.

use std::collections::BTreeMap;

use crate::color::Color;
use crate::names::camel;
use crate::palette::{Palette, Raised};

/// One theme's elevation, by QML name. The names are the same in every
/// theme.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Flat {
    /// `elevationCardBorder` and so on.
    pub borders: Vec<(String, Color)>,
    /// `elevationCardShadow1` and so on, in drawing order per level.
    pub layers: Vec<Layer>,
}

/// One shadow layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// The elevation level it belongs to (`card`).
    pub level: String,
    pub name: String,
    pub color: Color,
    pub y: f64,
    pub blur: f64,
}

/// `theme`'s elevation, padded to the shape every theme shares.
pub fn flatten(palette: &Palette, theme: &str) -> Flat {
    let mut flat = Flat::default();
    for (level, (count, has_border)) in shape(palette) {
        let raised: Option<&Raised> = palette.elevation.get(theme).and_then(|l| l.get(level));
        if has_border {
            flat.borders.push((
                camel(&format!("elevation.{level}.border")),
                raised.and_then(|r| r.border).unwrap_or_default(),
            ));
        }
        for i in 0..count {
            let shadow = raised.and_then(|r| r.shadows.get(i));
            flat.layers.push(Layer {
                level: level.to_owned(),
                name: camel(&format!("elevation.{level}.shadow{}", i + 1)),
                color: shadow.map(|s| s.color).unwrap_or_default(),
                y: shadow.map_or(0.0, |s| s.y),
                blur: shadow.map_or(0.0, |s| s.blur),
            });
        }
    }
    flat
}

/// Per level: the most shadow layers any theme has, and whether any theme
/// has a border.
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::palette::DropShadow;

    fn c(text: &str) -> Color {
        Color::parse(text).unwrap()
    }

    // covers: DS-01
    // suite: unit
    #[test]
    fn every_theme_gets_the_same_elevation_names() {
        let shadow = |y, color| DropShadow {
            y,
            blur: 2.0 * y,
            color: c(color),
        };
        let raised =
            |shadows, border| BTreeMap::from([("card".to_owned(), Raised { shadows, border })]);
        let palette = Palette {
            themes: BTreeMap::new(),
            elevation: BTreeMap::from([
                (
                    "light".to_owned(),
                    raised(
                        vec![shadow(1.0, "#00000010"), shadow(4.0, "#00000010")],
                        None,
                    ),
                ),
                ("dark".to_owned(), raised(vec![], Some(c("#34343D")))),
                (
                    "dim".to_owned(),
                    raised(vec![shadow(2.0, "#00000020")], None),
                ),
            ]),
        };
        let names = |flat: &Flat| -> Vec<String> {
            flat.borders
                .iter()
                .map(|(n, _)| n.clone())
                .chain(flat.layers.iter().map(|l| l.name.clone()))
                .collect()
        };
        let light = flatten(&palette, "light");
        let dark = flatten(&palette, "dark");
        let dim = flatten(&palette, "dim");
        let expected = [
            "elevationCardBorder",
            "elevationCardShadow1",
            "elevationCardShadow2",
        ];
        for flat in [&light, &dark, &dim] {
            assert_eq!(names(flat), expected);
        }
        assert_eq!(
            light.borders[0].1,
            Color::default(),
            "no border: transparent"
        );
        assert_eq!(dark.borders[0].1, c("#34343D"));
        assert_eq!(
            dark.layers[1].color,
            Color::default(),
            "no shadow: transparent"
        );
        assert_eq!(
            (dim.layers[0].y, dim.layers[1].y),
            (2.0, 0.0),
            "fewer layers: padded"
        );
    }
}
