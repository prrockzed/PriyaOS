//! The token file and the checks on it (0.10).

use std::path::Path;

use priya_themegen::color::Color;
use priya_themegen::contrast::{self, needed};
use priya_themegen::model::{ContrastLevel, Kind};
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

/// Reads and resolves token JSON, as `cargo xtask tokens` does.
fn resolve(value: &Value) -> Result<Palette, TokenError> {
    Palette::resolve(&parse(&value.to_string())?)
}

/// The contrast report for token JSON.
fn report(value: &Value) -> contrast::Report {
    let tokens = parse(&value.to_string()).expect("reads");
    contrast::check(&tokens, &Palette::resolve(&tokens).expect("resolves")).expect("checks")
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

/// The real file with one change.
fn with(change: impl FnOnce(&mut Value)) -> Value {
    let mut value = real();
    change(&mut value);
    value
}

// covers: DS-01
// suite: unit
// acceptance: 0.10
#[test]
fn tokens_json_validates_against_schema() {
    assert_eq!(schema_problems(&real()), Vec::<String>::new());
    load(tokens_path()).expect("the real token file reads");

    // The schema and the typed reading refuse the same mistakes.
    let mistakes = [
        with(|t| t["color"]["text.primary"]["light"] = json!("black")),
        with(|t| t["radius"]["card"] = json!("round")),
        with(|t| t["radius"]["card"] = json!(-4)),
        with(|t| t["surprise"] = json!(1)),
        with(|t| {
            t.as_object_mut().expect("an object").remove("version");
        }),
        with(|t| t["version"] = json!(2)),
        with(|t| t["type"]["body"]["weight"] = json!(700)),
        with(|t| t["focus"]["width"] = json!(0)),
        with(|t| t["elevation"]["card"]["dark"] = json!({})),
        with(|t| t["contrast"][0]["bg"] = json!([])),
        with(|t| t["color"]["focus"] = json!({"light": "#FF0000", "dark": "#FF0000"})),
        with(|t| t["color"]["mode.look"] = json!({"light": "#FF0000", "dark": "#FF0000"})),
        with(|t| t["themes"]["light"]["checked"] = json!(false)),
        with(|t| t["themes"]["dark"]["checked"] = json!(true)),
        with(|t| t["color"]["Text.x"] = json!({"light": "#000000", "dark": "#FFFFFF"})),
        with(|t| t["themes"]["HC"] = json!({"name": "High"})),
        with(|t| t["themes"]["oled"]["name"] = json!("")),
        with(|t| t["font"]["ui"] = json!("")),
        with(|t| t["elevation"]["card"]["light"] = json!({"shadow": [], "border": "#000000"})),
    ];
    for (i, bad) in mistakes.iter().enumerate() {
        assert!(!schema_problems(bad).is_empty(), "mistake {i}: schema");
        assert!(resolve(bad).is_err(), "mistake {i}: reading");
    }
    let Err(TokenError::Invalid(problems)) = parse(&mistakes[9].to_string()) else {
        panic!("an empty background list is refused");
    };
    assert_eq!(
        problems,
        ["contrast pair 1 (text.primary) has no background to check against"]
    );
    let radius = parse(&mistakes[1].to_string()).unwrap_err().to_string();
    assert!(
        radius.contains("a radius is pixels (0 or more) or \"full\""),
        "{radius}"
    );
}

// covers: DS-02
// suite: unit
// acceptance: 0.10
#[test]
fn every_color_has_light_and_dark() {
    let palette = resolve(&real()).unwrap();
    let names = palette.names();
    for theme in palette.themes.keys() {
        let here: Vec<&str> = palette.themes[theme].keys().map(String::as_str).collect();
        assert_eq!(
            here,
            names.iter().copied().collect::<Vec<_>>(),
            "{theme} has every name"
        );
    }
    // OLED gives only what differs; the rest comes from dark.
    assert_eq!(palette.get("oled", "bg.base"), Some(hex("#000000")));
    assert_eq!(
        palette.get("oled", "text.primary"),
        palette.get("dark", "text.primary")
    );

    let no_dark = with(|t| {
        t["color"]["info"].as_object_mut().unwrap().remove("dark");
    });
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
    assert_eq!(palette.get("oled", "accent.subtle"), Some(hex("#2A1F08")));
    assert_eq!(
        palette.get("light", "mode.sandbox"),
        palette.get("light", "success")
    );
    assert_eq!(palette.get("dark", "focus"), palette.get("dark", "accent"));
    let card = |theme: &str| palette.elevation[theme]["card"].clone();
    assert_eq!(
        card("light").shadows.len(),
        2,
        "the approved two-layer shadow"
    );
    assert_eq!(card("light").shadows[1].blur, 12.0);
    assert_eq!(card("dark").border, palette.get("dark", "border.subtle"));
    assert_eq!(card("oled").border, palette.get("oled", "border.subtle"));

    // The default accent gives the palette's accent colours, and every
    // other option must be whole.
    let indigo = json!({
        "name": "Indigo",
        "accent": {"light": "#3F3FBF", "dark": "#A5A5FF"},
        "accent.subtle": {"light": "#EEEEFF", "dark": "#22224A"},
        "on.accent": {"light": "#FFFFFF", "dark": "#10102A"}
    });
    let switched = with(|t| {
        t["accent"]["options"]["indigo"] = indigo.clone();
        t["accent"]["default"] = json!("indigo");
    });
    assert_eq!(
        resolve(&switched).unwrap().get("light", "accent"),
        Some(hex("#3F3FBF"))
    );
    let broken_option = with(|t| {
        t["accent"]["options"]["indigo"] = indigo.clone();
        t["accent"]["options"]["indigo"]["accent"]["light"] = json!("nope");
    });
    assert!(matches!(
        resolve(&broken_option),
        Err(TokenError::BadColor { token, .. }) if token == "accent.indigo.accent"
    ));
    // A broken default is named as the file names it.
    let broken_default =
        with(|t| t["accent"]["options"]["marigold"]["accent"]["light"] = json!("nope"));
    assert!(
        resolve(&broken_default)
            .unwrap_err()
            .to_string()
            .starts_with("`accent.marigold.accent` in `light` is `nope`")
    );

    let circle = with(|t| {
        t["color"]["warning"]["light"] = json!("{color.success}");
        t["color"]["success"]["light"] = json!("{color.info}");
        t["color"]["info"]["light"] = json!("{color.success}");
    });
    match resolve(&circle) {
        Err(TokenError::ReferenceCycle { theme, chain }) => {
            assert_eq!(theme, "light");
            assert_eq!(chain, ["color.info", "color.success", "color.info"]);
        }
        other => panic!("{other:?}"),
    }

    let unknown = with(|t| t["mode"]["look"] = json!("{color.nope}"));
    assert_eq!(
        resolve(&unknown).unwrap_err().to_string(),
        "`mode.look` in `dark` refers to `color.nope`, which doesn't exist"
    );
    let themes_circle = with(|t| t["themes"]["dark"]["inherits"] = json!("oled"));
    assert!(matches!(
        resolve(&themes_circle),
        Err(TokenError::InheritanceCycle(_))
    ));
    let stray = with(|t| t["color"]["info"]["sepia"] = json!("#000000"));
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
    assert_eq!(needed(Kind::Text, ContrastLevel::Normal), 4.5);
    assert_eq!(needed(Kind::LargeText, ContrastLevel::Normal), 3.0);
    assert_eq!(needed(Kind::NonText, ContrastLevel::Normal), 3.0);
    assert_eq!(needed(Kind::Text, ContrastLevel::High), 7.0);
    assert_eq!(needed(Kind::LargeText, ContrastLevel::High), 4.5);
    assert_eq!(needed(Kind::NonText, ContrastLevel::High), 3.0);

    // #8A8A8A on white is about 3.45:1: too low for text, enough for large.
    let tokens = with(|t| {
        t["color"]["text.tertiary"]["light"] = json!("#8A8A8A");
        t["contrast"] = json!([
            {"fg": "text.tertiary", "bg": ["bg.surface1"], "kind": "text"},
            {"fg": "text.tertiary", "bg": ["bg.surface1"], "kind": "large-text"},
            {"fg": "text.primary", "bg": ["bg.base"], "kind": "text"},
            {"fg": "text.secondary", "bg": ["bg.base"], "kind": "text"},
            {"fg": "on.accent", "bg": ["accent"], "kind": "text"}
        ]);
    });
    let low = report(&tokens);
    let failed: Vec<(String, Kind)> = low
        .measured
        .iter()
        .filter(|m| !m.passes())
        .map(|m| (m.theme.clone(), m.kind))
        .collect();
    assert_eq!(failed, [("light".to_owned(), Kind::Text)]);
    assert!(!low.passes());
    assert_eq!(
        low.problems(),
        ["text.tertiary on bg.surface1 in light: 3.45:1, needs 4.5:1"]
    );
    assert!(
        low.to_markdown()
            .contains("| `text.tertiary` | `bg.surface1` | text | 3.45:1 | 4.5:1 | ✗ |")
    );

    // Optional themes are checked only when asked, high contrast to AAA.
    let themes = |r: &contrast::Report| {
        let mut themes: Vec<String> = r.measured.iter().map(|m| m.theme.clone()).collect();
        themes.dedup();
        themes
    };
    assert_eq!(themes(&low), ["dark", "light"]);
    let high = with(|t| {
        t["themes"]["oled"]["checked"] = json!(true);
        t["themes"]["oled"]["contrast"] = json!("high");
    });
    let checked = report(&high);
    assert!(themes(&checked).contains(&"oled".to_owned()));
    assert!(
        checked
            .measured
            .iter()
            .any(|m| m.theme == "oled" && m.needed == 7.0)
    );
}

// covers: DS-06
// suite: unit
// acceptance: 0.10
#[test]
fn undeclared_text_token_flagged() {
    let ghost = with(|t| t["color"]["text.ghost"] = json!({"light": "#000000", "dark": "#FFFFFF"}));
    assert_eq!(report(&ghost).unchecked_text, ["text.ghost"]);
    assert!(!report(&ghost).passes());
    // Checking it only as a border doesn't count.
    let as_border = with(|t| {
        t["color"]["text.ghost"] = json!({"light": "#000000", "dark": "#FFFFFF"});
        t["contrast"]
            .as_array_mut()
            .unwrap()
            .push(json!({"fg": "text.ghost", "bg": ["bg.base"], "kind": "non-text"}));
    });
    assert_eq!(report(&as_border).unchecked_text, ["text.ghost"]);

    // The accent's label colour counts too, though it isn't in `color`.
    let no_label = with(|t| {
        t["contrast"]
            .as_array_mut()
            .unwrap()
            .retain(|pair| pair["fg"] != "on.accent");
    });
    assert_eq!(report(&no_label).unchecked_text, ["on.accent"]);

    let unknown = with(|t| t["contrast"][0]["fg"] = json!("text.nope"));
    let tokens = parse(&unknown.to_string()).unwrap();
    assert_eq!(
        contrast::check(&tokens, &Palette::resolve(&tokens).unwrap())
            .unwrap_err()
            .to_string(),
        "a contrast pair names `text.nope`, which isn't a colour in `dark`"
    );
    let see_through = with(|t| t["color"]["bg.base"]["light"] = json!("#FAFAF780"));
    let tokens = parse(&see_through.to_string()).unwrap();
    assert!(matches!(
        contrast::check(&tokens, &Palette::resolve(&tokens).unwrap()),
        Err(TokenError::TranslucentBase(theme)) if theme == "light"
    ));
}

// covers: DS-02, DS-06
// suite: unit
// acceptance: 0.10
#[test]
fn the_shipped_palette_passes_and_matches_the_design_doc() {
    let shipped = report(&real());
    assert!(shipped.passes(), "{:#?}", shipped.problems());
    let palette = resolve(&real()).unwrap();

    // 11 §2.1's table shows the same values: | `token` … | light | dark | oled |
    let doc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/claude/architecture/11-design-system-and-ux.md"
    ))
    .expect("the design doc");
    let mut rows = 0;
    for line in doc.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let Some(token) = cells.get(1).and_then(|c| c.split('`').nth(1)) else {
            continue;
        };
        if palette.get("light", token).is_none() {
            continue;
        }
        assert!(
            cells.len() >= 6,
            "11 §2.1's row for {token} has three values"
        );
        for (theme, cell) in ["light", "dark", "oled"].iter().zip(&cells[2..5]) {
            let shown = Color::parse(cell.trim_matches('`'))
                .unwrap_or_else(|| panic!("11 §2.1 shows {cell} for {token} in {theme}"));
            assert_eq!(palette.get(theme, token), Some(shown), "{token} in {theme}");
        }
        rows += 1;
    }
    let tokens = load(tokens_path()).unwrap();
    assert_eq!(
        rows,
        tokens.color.len() + 3,
        "every colour and the accent's three are in the table"
    );
}
