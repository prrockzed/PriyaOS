//! `ui/design/build/tokens.css`: the tokens as CSS custom properties, so
//! HTML mockups use the very values the desktop does. Colours are set per
//! `[data-theme]`; light is also the default.

use std::fmt::Write as _;

use crate::GENERATED;
use crate::elevation::flatten;
use crate::model::{Radius, Tokens};
use crate::names::kebab;
use crate::palette::Palette;

pub fn tokens_css(tokens: &Tokens, palette: &Palette) -> String {
    let mut out = format!("/* {GENERATED} */\n");
    // Light first: `:root` weighs as much as `[data-theme]`, so a later
    // rule must be the one a page asks for.
    let mut themes: Vec<&String> = palette.themes.keys().collect();
    themes.sort_by_key(|theme| *theme != "light");
    for theme in themes {
        let selector = if theme == "light" {
            ":root, [data-theme=\"light\"]".to_owned()
        } else {
            format!("[data-theme=\"{theme}\"]")
        };
        let _ = writeln!(out, "\n{selector} {{");
        for (name, color) in &palette.themes[theme] {
            let _ = writeln!(out, "  --{}: {color};", kebab(name));
        }
        let flat = flatten(palette, theme);
        for (name, color) in &flat.borders {
            let _ = writeln!(out, "  --{}: {color};", kebab(name));
        }
        let layers = flat.layers;
        for level in tokens.elevation.keys() {
            let shadows: Vec<String> = layers
                .iter()
                .filter(|layer| layer.level == *level && layer.color.a > 0)
                .map(|layer| format!("0 {}px {}px {}", layer.y, layer.blur, layer.color))
                .collect();
            let value = if shadows.is_empty() {
                "none".to_owned()
            } else {
                shadows.join(", ")
            };
            let _ = writeln!(out, "  --elevation-{level}-shadow: {value};");
        }
        out.push_str("}\n");
    }

    out.push_str("\n:root {\n");
    let fonts = &tokens.font;
    let _ = writeln!(
        out,
        "  --font-ui: \"{}\", \"{}\", sans-serif;",
        fonts.ui, fonts.indic
    );
    let _ = writeln!(out, "  --font-mono: \"{}\", monospace;", fonts.mono);
    for (name, style) in tokens.type_scale.all() {
        let name = kebab(name);
        let _ = writeln!(out, "  --type-{name}-size: {}px;", style.size);
        let _ = writeln!(out, "  --type-{name}-weight: {};", style.weight);
        let _ = writeln!(out, "  --type-{name}-line-height: {};", style.line_height);
    }
    for step in &tokens.space {
        let _ = writeln!(out, "  --space-{step}: {step}px;");
    }
    for (name, radius) in tokens.radius.all() {
        let value = match radius {
            Radius::Pixels(px) => format!("{px}px"),
            Radius::Full => "9999px".to_owned(),
        };
        let _ = writeln!(out, "  --radius-{name}: {value};");
    }
    let motion = &tokens.motion;
    for (name, ms) in [
        ("micro", motion.micro),
        ("standard", motion.standard),
        ("panel", motion.panel),
    ] {
        let _ = writeln!(out, "  --motion-{name}: {ms}ms;");
    }
    for (name, [x1, y1, x2, y2]) in [
        ("standard", motion.easing.standard),
        ("emphasised", motion.easing.emphasised),
    ] {
        let _ = writeln!(
            out,
            "  --easing-{name}: cubic-bezier({x1}, {y1}, {x2}, {y2});"
        );
    }
    let _ = writeln!(out, "  --focus-width: {}px;", tokens.focus.width);
    out.push_str("}\n");
    out
}
