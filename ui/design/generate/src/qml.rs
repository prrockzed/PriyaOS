//! `ui/PriyaUI/Theme.qml`: the singleton every PriyaUI component reads its
//! colours, type, spacing, radius and motion from (11 §8: nothing else may
//! hard-code them).

use crate::GENERATED;
use crate::elevation::{Layer, borders, layers};
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
    colors_in_use(&mut out, palette);
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

/// One object per theme, each with every name, so switching never meets a
/// missing property: what a theme doesn't use (a dark theme's shadows) is
/// transparent, or 0.
fn palettes(out: &mut Qml, palette: &Palette) {
    let names = palette.names();
    for theme in palette.themes.keys() {
        out.line(&format!(
            "    readonly property QtObject {}: QtObject {{",
            camel(theme)
        ));
        for name in &names {
            let color = palette.get(theme, name).unwrap_or_default();
            out.line(&format!(
                "        readonly property color {}: \"{color}\"",
                qml_property(name)
            ));
        }
        for (name, color) in borders(palette, theme) {
            out.line(&format!(
                "        readonly property color {name}: \"{color}\""
            ));
        }
        for layer in layers(palette, theme) {
            let Layer {
                name,
                color,
                y,
                blur,
                ..
            } = layer;
            out.line(&format!(
                "        readonly property color {name}: \"{color}\""
            ));
            out.line(&format!("        readonly property real {name}Y: {y}"));
            out.line(&format!(
                "        readonly property real {name}Blur: {blur}"
            ));
        }
        out.line("    }");
    }
    let lookup: Vec<String> = palette
        .themes
        .keys()
        .map(|t| format!("\"{t}\": {}", camel(t)))
        .collect();
    out.line(&format!(
        "    readonly property QtObject palette: ({{ {} }})[mode] ?? {}",
        lookup.join(", "),
        camel(FALLBACK_THEME)
    ));
    out.line("");
}

/// The colours in use, which ease into a new mode.
fn colors_in_use(out: &mut Qml, palette: &Palette) {
    out.line("    // The colours in use. They change smoothly when the mode does.");
    let animated = |out: &mut Qml, name: &str| {
        out.line(&format!("    property color {name}: palette.{name}"));
        out.line(&format!(
            "    Behavior on {name} {{ enabled: theme.animationsEnabled; ColorAnimation {{ duration: theme.motionStandard }} }}"
        ));
    };
    for name in palette.names() {
        animated(out, &qml_property(name));
    }
    out.line("    // Elevation: shadow layers (light) and borders (dark).");
    if let Some(theme) = palette.themes.keys().next() {
        for (name, _) in borders(palette, theme) {
            animated(out, &name);
        }
        for Layer { name, .. } in layers(palette, theme) {
            animated(out, &name);
            out.line(&format!(
                "    readonly property real {name}Y: palette.{name}Y * scale"
            ));
            out.line(&format!(
                "    readonly property real {name}Blur: palette.{name}Blur * scale"
            ));
        }
    }
    out.line("");
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
