//! `ui/PriyaUI/Theme.qml`: the singleton every PriyaUI component reads its
//! colours, type, spacing, radius and motion from (11 §8: nothing else may
//! hard-code them).

use std::collections::BTreeSet;

use crate::GENERATED;
use crate::color::Color;
use crate::elevation::{Flat, Layer, flatten};
use crate::model::{Radius, Tokens};
use crate::names::{camel, qml_property};
use crate::palette::Palette;

/// The palette shown when `mode` names no theme: dark, as at night.
const FALLBACK_THEME: &str = "dark";

pub fn theme_qml(tokens: &Tokens, palette: &Palette) -> String {
    let mut out = Qml::default();
    out.line(&format!("// {GENERATED}"));
    out.line("pragma Singleton");
    out.line("import QtQuick");
    out.line("");
    out.line("QtObject {");
    out.line("    id: theme");
    out.line("");
    settings(&mut out, tokens);
    palettes(&mut out, palette);
    type_scale(&mut out, tokens);
    sizes(&mut out, tokens);
    motion(&mut out, tokens);
    out.line("}");
    out.0
}

#[derive(Default)]
struct Qml(String);

impl Qml {
    fn line(&mut self, text: &str) {
        self.0.push_str(text);
        self.0.push('\n');
    }
}

fn settings(out: &mut Qml, tokens: &Tokens) {
    let themes: Vec<String> = tokens.themes.keys().map(|t| format!("\"{t}\"")).collect();
    out.line(&format!(
        "    // Which palette is shown: {}.",
        themes.join(", ")
    ));
    out.line(&format!("    property string mode: \"{FALLBACK_THEME}\""));
    out.line("    // Movement off (the user's setting or the system's): changes are instant.");
    out.line("    property bool reducedMotion: false");
    out.line("    readonly property bool animationsEnabled: !reducedMotion");
    out.line("    // Interface scale; every size below is multiplied by it.");
    out.line("    property real scale: 1.0");
    out.line("");
}

/// One `Palette` per theme. The component declares every name, so a theme
/// can't miss one, lookups are typed, and what a theme doesn't use (a dark
/// theme's shadows) is transparent, or 0.
fn palettes(out: &mut Qml, palette: &Palette) {
    let names = palette.names();
    let shape = palette
        .themes
        .keys()
        .next()
        .map(|theme| flatten(palette, theme))
        .unwrap_or_default();
    out.line("    // What every theme's palette has.");
    out.line("    component Palette: QtObject {");
    for name in &names {
        out.line(&format!("        property color {}", qml_property(name)));
    }
    for (name, _) in &shape.borders {
        out.line(&format!("        property color {name}"));
    }
    for Layer { name, .. } in &shape.layers {
        out.line(&format!("        property color {name}"));
        out.line(&format!("        property real {name}Y"));
        out.line(&format!("        property real {name}Blur"));
    }
    out.line("    }");
    for theme in palette.themes.keys() {
        out.line(&format!(
            "    readonly property Palette {}: Palette {{",
            camel(theme)
        ));
        for name in &names {
            let color = palette.get(theme, name).unwrap_or_default();
            out.line(&format!(
                "        {}: \"{}\"",
                qml_property(name),
                qml_color(color)
            ));
        }
        let flat = flatten(palette, theme);
        for (name, color) in &flat.borders {
            out.line(&format!("        {name}: \"{}\"", qml_color(*color)));
        }
        for Layer {
            name,
            color,
            y,
            blur,
            ..
        } in &flat.layers
        {
            out.line(&format!("        {name}: \"{}\"", qml_color(*color)));
            out.line(&format!("        {name}Y: {y}"));
            out.line(&format!("        {name}Blur: {blur}"));
        }
        out.line("    }");
    }
    // A conditional chain keeps the type: `palette.bgBase` is checked.
    let mut choice = camel(FALLBACK_THEME);
    for theme in palette.themes.keys().rev().filter(|t| *t != FALLBACK_THEME) {
        choice = format!("mode === \"{theme}\" ? {} : {choice}", camel(theme));
    }
    out.line(&format!("    readonly property Palette palette: {choice}"));
    out.line("");
    colors_in_use(out, &names, &shape);
}

/// The colours in use, which ease into a new mode.
fn colors_in_use(out: &mut Qml, names: &BTreeSet<&str>, shape: &Flat) {
    out.line("    // The colours in use. They change smoothly when the mode does.");
    let animated = |out: &mut Qml, name: &str| {
        out.line(&format!("    property color {name}: palette.{name}"));
        out.line(&format!(
            "    Behavior on {name} {{ enabled: theme.animationsEnabled; ColorAnimation {{ duration: theme.motionStandard }} }}"
        ));
    };
    for name in names {
        animated(out, &qml_property(name));
    }
    out.line("    // Elevation: shadow layers (light) and borders (dark).");
    for (name, _) in &shape.borders {
        animated(out, name);
    }
    for Layer { name, .. } in &shape.layers {
        animated(out, name);
        out.line(&format!(
            "    readonly property real {name}Y: palette.{name}Y * scale"
        ));
        out.line(&format!(
            "    readonly property real {name}Blur: palette.{name}Blur * scale"
        ));
    }
    out.line("");
}

/// QML reads 8-digit colours as `#AARRGGBB`, unlike CSS.
pub fn qml_color(color: Color) -> String {
    if color.is_opaque() {
        format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.a, color.r, color.g, color.b
        )
    }
}

fn type_scale(out: &mut Qml, tokens: &Tokens) {
    let fonts = &tokens.font;
    out.line(&format!(
        "    readonly property string fontUi: \"{}\"",
        fonts.ui
    ));
    out.line(&format!(
        "    readonly property string fontMono: \"{}\"",
        fonts.mono
    ));
    out.line(&format!(
        "    readonly property string fontIndic: \"{}\"",
        fonts.indic
    ));
    // QML fonts take one family; fontconfig supplies the fallback for
    // Devanagari (11 §2.2), which is why fontIndic is named here too.
    out.line("    // Type styles. Devanagari text falls back to fontIndic through fontconfig.");
    for (name, style) in tokens.type_scale.all() {
        out.line(&format!(
            "    readonly property font {name}: Qt.font({{ family: fontUi, pixelSize: Math.round({} * scale), weight: {} }})",
            style.size, style.weight
        ));
        out.line(&format!(
            "    readonly property real {name}LineHeight: {}",
            style.line_height
        ));
    }
    out.line("");
}

fn sizes(out: &mut Qml, tokens: &Tokens) {
    out.line("    // Spacing on a 4 px grid: space8 is 8 px at scale 1.");
    for step in &tokens.space {
        out.line(&format!(
            "    readonly property real space{step}: {step} * scale"
        ));
    }
    out.line("    // Corner radii. Qt clamps a radius to half the height, so a pill's is large.");
    for (name, radius) in tokens.radius.all() {
        let value = match radius {
            Radius::Pixels(px) => format!("{px} * scale"),
            Radius::Full => "9999".to_owned(),
        };
        out.line(&format!(
            "    readonly property real {}: {value}",
            camel(&format!("radius.{name}"))
        ));
    }
    out.line(&format!(
        "    readonly property real focusWidth: {} * scale",
        tokens.focus.width
    ));
    out.line("");
}

fn motion(out: &mut Qml, tokens: &Tokens) {
    let motion = &tokens.motion;
    out.line("    // Durations in milliseconds; 0 when movement is off.");
    for (name, ms) in [
        ("motionMicro", motion.micro),
        ("motionStandard", motion.standard),
        ("motionPanel", motion.panel),
    ] {
        out.line(&format!(
            "    readonly property int {name}: animationsEnabled ? {ms} : 0"
        ));
    }
    out.line("    // For easing.type Easing.BezierSpline: two control points, then the end.");
    for (name, [x1, y1, x2, y2]) in [
        ("easingStandard", motion.easing.standard),
        ("easingEmphasised", motion.easing.emphasised),
    ] {
        out.line(&format!(
            "    readonly property var {name}: [{x1}, {y1}, {x2}, {y2}, 1, 1]"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: DS-01
    // suite: unit
    #[test]
    fn qml_colors_put_alpha_first() {
        let c = |text| Color::parse(text).unwrap();
        assert_eq!(qml_color(c("#18181B0F")), "#0F18181B");
        assert_eq!(qml_color(c("#FAFAF7")), "#FAFAF7");
        assert_eq!(qml_color(Color::default()), "#00000000");
    }
}
