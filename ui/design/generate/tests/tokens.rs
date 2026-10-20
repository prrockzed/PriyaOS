//! The token file and the checks on it (0.10).

use std::path::Path;

use priya_themegen::color::Color;
use priya_themegen::contrast::{self, needed};
use priya_themegen::model::Kind;
use priya_themegen::{Palette, TokenError, load, parse};
use serde_json::{Value, json};

fn tokens_path() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../tokens.json"))
}

/// The real token file as JSON, to change in a test.
fn real() -> Value {
    let text = std::fs::read_to_string(tokens_path()).expect("the token file");
    serde_json::from_str(&text).expect("JSON")
}

fn resolve(value: &Value) -> Result<Palette, TokenError> {
    Palette::resolve(&parse(&value.to_string())?)
}

fn hex(text: &str) -> Color {
    Color::parse(text).expect("a colour")
}

/// What the schema says is wrong with `value`, by place.
fn schema_problems(value: &Value) -> Vec<String> {
    let schema: Value = serde_json::from_str(priya_themegen::SCHEMA).expect("JSON");
    let validator = jsonschema::validator_for(&schema).expect("a valid schema");
    validator
        .iter_errors(value)
        .map(|e| format!("{} {e}", e.instance_path()))
        .collect()
}

// covers: DS-01
// suite: unit
// acceptance: 0.10
#[test]
fn tokens_json_validates_against_schema() {
    assert_eq!(schema_problems(&real()), Vec::<String>::new());
    load(tokens_path()).expect("the real token file reads");

    // The schema and the typed reading refuse the same mistakes.
    let mut bad_color = real();
    bad_color["color"]["text.primary"]["light"] = json!("black");
    let mut bad_radius = real();
    bad_radius["radius"]["card"] = json!("round");
    let mut surprise = real();
    surprise["surprise"] = json!(1);
    let mut no_version = real();
    no_version
        .as_object_mut()
        .expect("an object")
        .remove("version");
    for (bad, place) in [
        (bad_color, "/color/text.primary/light"),
        (bad_radius, "/radius/card"),
        (surprise, "surprise"),
        (no_version, "version"),
    ] {
        let problems = schema_problems(&bad).join("\n");
        assert!(problems.contains(place), "{place} in {problems}");
        let read = parse(&bad.to_string()).and_then(|t| Palette::resolve(&t));
        assert!(read.is_err(), "{place} is refused when read too");
    }
}

// covers: DS-02
// suite: unit
// acceptance: 0.10
#[test]
fn every_color_has_light_and_dark() {
    let tokens = load(tokens_path()).unwrap();
    let palette = Palette::resolve(&tokens).unwrap();
    for theme in ["light", "dark"] {
        for name in tokens.color.keys() {
            assert!(palette.get(theme, name).is_some(), "{name} in {theme}");
        }
    }
    // OLED gives only what differs; the rest comes from dark.
    assert_eq!(palette.get("oled", "bg.base"), Some(hex("#000000")));
    assert_eq!(
        palette.get("oled", "text.primary"),
        palette.get("dark", "text.primary")
    );

    let mut no_dark = real();
    no_dark["color"]["info"]
        .as_object_mut()
        .unwrap()
        .remove("dark");
    assert!(!schema_problems(&no_dark).is_empty());
    assert_eq!(
        resolve(&no_dark).unwrap_err().to_string(),
        "`color.info` has no value for theme `dark`"
    );
}

// covers: DS-01
// suite: unit
// acceptance: 0.10
#[test]
fn references_resolve_and_cycles_rejected() {
    let palette = resolve(&real()).unwrap();
    assert_eq!(palette.get("light", "accent"), Some(hex("#915500")));
    assert_eq!(palette.get("dark", "on.accent"), Some(hex("#1A1300")));
    assert_eq!(
        palette.get("light", "mode.sandbox"),
        palette.get("light", "success")
    );
    assert_eq!(palette.get("dark", "focus"), palette.get("dark", "accent"));
    assert_eq!(
        palette.get("dark", "elevation.raised.border"),
        palette.get("dark", "border.subtle")
    );

    let mut circle = real();
    circle["color"]["success"]["light"] = json!("{color.info}");
    circle["color"]["info"]["light"] = json!("{color.success}");
    match resolve(&circle) {
        Err(TokenError::ReferenceCycle { theme, chain }) => {
            assert_eq!(theme, "light");
            assert_eq!(chain.first(), chain.last(), "{chain:?}");
        }
        other => panic!("{other:?}"),
    }

    let mut unknown = real();
    unknown["mode"]["look"] = json!("{color.nope}");
    let err = resolve(&unknown).unwrap_err();
    assert_eq!(
        err.to_string(),
        "`mode.look` in `dark` refers to `color.nope`, which doesn't exist"
    );

    let mut themes_circle = real();
    themes_circle["themes"]["dark"]["inherits"] = json!("oled");
    assert!(matches!(
        resolve(&themes_circle),
        Err(TokenError::InheritanceCycle(_))
    ));

    let mut stray = real();
    stray["color"]["info"]["sepia"] = json!("#000000");
    assert!(matches!(
        resolve(&stray),
        Err(TokenError::ValueForUnknownTheme { .. })
    ));
}

// covers: DS-06
// suite: unit
// acceptance: 0.10
#[test]
fn contrast_check_fails_below_threshold_per_kind() {
    assert_eq!(needed(Kind::Text, "light"), 4.5);
    assert_eq!(needed(Kind::LargeText, "dark"), 3.0);
    assert_eq!(needed(Kind::NonText, "light"), 3.0);
    assert_eq!(needed(Kind::Text, "hc-dark"), 7.0);

    let mut tokens = real();
    tokens["color"]["text.tertiary"]["light"] = json!("#8A8A8A");
    tokens["contrast"] = json!([
        {"fg": "text.tertiary", "bg": ["bg.surface1"], "kind": "text"},
        {"fg": "text.tertiary", "bg": ["bg.surface1"], "kind": "large-text"},
        {"fg": "text.primary", "bg": ["bg.base"], "kind": "text"},
        {"fg": "text.secondary", "bg": ["bg.base"], "kind": "text"},
        {"fg": "on.accent", "bg": ["accent"], "kind": "text"}
    ]);
    let parsed = parse(&tokens.to_string()).unwrap();
    let report = contrast::check(&parsed, &Palette::resolve(&parsed).unwrap()).unwrap();
    let failed: Vec<(String, Kind)> = report
        .measured
        .iter()
        .filter(|m| !m.passes())
        .map(|m| (m.theme.clone(), m.kind))
        .collect();
    // #8A8A8A on white is about 3.45:1: too low for text, enough for large.
    assert_eq!(failed, [("light".to_owned(), Kind::Text)]);
    assert!(!report.passes());
    assert_eq!(
        report.problems(),
        ["text.tertiary on bg.surface1 in light: 3.45:1, needs 4.5:1"]
    );
    assert!(
        report
            .to_markdown()
            .contains("| `text.tertiary` | `bg.surface1` | text | 3.45:1 | 4.5:1 | ✗ |")
    );
}

// covers: DS-06
// suite: unit
// acceptance: 0.10
#[test]
fn undeclared_text_token_flagged() {
    let mut tokens = real();
    tokens["color"]["text.ghost"] = json!({"light": "#000000", "dark": "#FFFFFF"});
    let parsed = parse(&tokens.to_string()).unwrap();
    let report = contrast::check(&parsed, &Palette::resolve(&parsed).unwrap()).unwrap();
    assert_eq!(report.unchecked_text, ["text.ghost"]);
    assert!(!report.passes());

    let mut unknown = real();
    unknown["contrast"][0]["fg"] = json!("text.nope");
    let parsed = parse(&unknown.to_string()).unwrap();
    assert!(matches!(
        contrast::check(&parsed, &Palette::resolve(&parsed).unwrap()),
        Err(TokenError::UnknownPairColor(name)) if name == "text.nope"
    ));
}

// covers: DS-02, DS-06
// suite: unit
// acceptance: 0.10
#[test]
fn the_shipped_palette_passes_and_matches_the_design_doc() {
    let tokens = load(tokens_path()).unwrap();
    let palette = Palette::resolve(&tokens).unwrap();
    let report = contrast::check(&tokens, &palette).unwrap();
    assert!(report.passes(), "{:#?}", report.problems());
    assert!(report.measured.iter().any(|m| m.theme == "light"));
    assert!(report.measured.iter().any(|m| m.theme == "dark"));

    // 11 §2.1's table shows the same values: | `token` … | light | dark | oled |
    let doc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/claude/architecture/11-design-system-and-ux.md"
    ))
    .unwrap();
    let mut rows = 0;
    for line in doc.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let token = cells[1].split('`').nth(1).unwrap();
        if !tokens.color.contains_key(token) {
            continue;
        }
        for (theme, cell) in ["light", "dark", "oled"].iter().zip(&cells[2..5]) {
            let shown = hex(cell.trim_matches('`'));
            assert_eq!(palette.get(theme, token), Some(shown), "{token} in {theme}");
        }
        rows += 1;
    }
    assert_eq!(
        rows,
        tokens.color.len(),
        "every colour token is in the table"
    );
}
