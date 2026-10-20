//! What the generators make from the real token file (0.10). The committed
//! files are the goldens: `cargo xtask tokens` rewrites them, and CI's
//! `cargo xtask tokens --check` fails when they drift.

use std::path::Path;

use priya_themegen::{GENERATED, Outputs, Palette, generate, load};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");

fn outputs() -> (Outputs, Palette) {
    let tokens = load(&Path::new(ROOT).join("ui/design/tokens.json")).expect("the tokens");
    let palette = Palette::resolve(&tokens).expect("resolves");
    (generate(&tokens).expect("generates"), palette)
}

fn committed(path: &str) -> String {
    std::fs::read_to_string(Path::new(ROOT).join(path))
        .unwrap_or_else(|e| panic!("{path}: {e}; run `cargo xtask tokens`"))
}

// covers: DS-01
// suite: unit
// acceptance: 0.10
#[test]
fn theme_qml_generation_golden() {
    let (outputs, palette) = outputs();
    let qml = &outputs.theme_qml;
    assert_eq!(
        *qml,
        committed("ui/PriyaUI/Theme.qml"),
        "run `cargo xtask tokens`"
    );
    assert_eq!(qml.lines().next(), Some(format!("// {GENERATED}").as_str()));
    assert!(qml.contains("pragma Singleton"));
    // Every theme is a typed Palette that sets every name.
    for theme in ["dark", "light", "oled"] {
        assert!(qml.contains(&format!("readonly property Palette {theme}: Palette {{")));
    }
    let count = |needle: &str| qml.matches(needle).count();
    for name in [
        "bgBase",
        "textOnAccent",
        "modeDirectRaw",
        "elevationCardShadow1",
    ] {
        assert_eq!(
            count(&format!("        property color {name}\n")),
            1,
            "{name} declared"
        );
        assert_eq!(
            count(&format!("        {name}: \"")),
            palette.themes.len(),
            "{name} in every theme"
        );
        assert_eq!(count(&format!("Behavior on {name} ")), 1, "{name} animates");
    }
    // QML colours take alpha first: #18181B at 6% is #0F18181B.
    assert!(qml.contains("elevationCardShadow1: \"#0F18181B\""));
    assert!(qml.contains(
        "readonly property Palette palette: mode === \"light\" ? light : mode === \"oled\" ? oled : dark"
    ));
    // QML reads `onX` as a signal handler.
    assert!(!qml.contains("property color on") && !qml.contains("        on"));
}

// covers: DS-01
// suite: unit
// acceptance: 0.10
#[test]
fn tokens_css_generation_golden() {
    let (outputs, _) = outputs();
    let css = &outputs.tokens_css;
    assert_eq!(
        *css,
        committed("ui/design/build/tokens.css"),
        "run `cargo xtask tokens`"
    );
    assert_eq!(
        css.lines().next(),
        Some(format!("/* {GENERATED} */").as_str())
    );
    // Light is the default, so it comes first: later rules must win.
    let light = css.find(":root, [data-theme=\"light\"]").expect("light");
    let dark = css.find("[data-theme=\"dark\"]").expect("dark");
    assert!(light < dark);
    assert!(css.contains("--on-accent: #FFFFFF;"));
    assert!(css.contains("--elevation-card-shadow: 0 1px 2px #18181B0F, 0 4px 12px #18181B0F;"));
    assert!(css.contains("--easing-standard: cubic-bezier(0.2, 0, 0, 1);"));
}

// covers: DS-06
// suite: unit
// acceptance: 0.10
#[test]
fn the_contrast_report_lists_every_pair_passing() {
    let (outputs, _) = outputs();
    let report = outputs.contrast.to_markdown();
    assert!(report.contains("## light") && report.contains("## dark"));
    assert!(!report.contains('✗'), "{report}");
    assert_eq!(report.matches('✓').count(), outputs.contrast.measured.len());
}
