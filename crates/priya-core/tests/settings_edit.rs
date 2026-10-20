//! Editing a comment-heavy settings file (0.6): only the changed lines may
//! differ, and the results are checked against golden files.

use std::path::Path;

use priya_core::settings::{KeyPath, set, unset};
use priya_testkit::{assert_golden_text, fixture};
use serde_json::json;

fn original() -> String {
    let dir = fixture(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/settings-edit"));
    std::fs::read_to_string(dir.join("commented.toml")).expect("the fixture is readable")
}

fn key(text: &str) -> KeyPath {
    KeyPath::parse(text).expect("a valid key")
}

/// The lines of `after` that differ from `before`, which must have as many.
fn changed_lines(before: &str, after: &str) -> Vec<(String, String)> {
    assert_eq!(before.lines().count(), after.lines().count(), "{after}");
    before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .collect()
}

// covers: CORE-03
// suite: unit
// acceptance: 0.6
// regression: 0.6
#[test]
fn set_preserves_comments_and_order() {
    let before = original();

    let theme = set(&before, &key("ui.theme"), &json!("light")).unwrap();
    assert_eq!(
        changed_lines(&before, &theme),
        [(
            r#"theme   = "dark"        # light or dark"#.to_owned(),
            r#"theme   = "light"        # light or dark"#.to_owned()
        )],
        "byte-identical apart from the changed line"
    );
    assert_golden_text!("set_ui_theme", theme);

    for (name, edit, value) in [
        (
            "set_base_url",
            "ai.providers.ollama.base_url",
            json!("http://127.0.0.1:11434/v1"),
        ),
        (
            "set_enabled",
            "modules.enabled",
            json!(["weather", "clock"]),
        ),
        ("set_city", "modules.weather.city", json!("Patna")),
    ] {
        let after = set(&before, &key(edit), &value).unwrap();
        assert_eq!(changed_lines(&before, &after).len(), 1, "{name}");
        assert_golden_text!(name, after);
    }

    // Adding keys and sections, and removing them, keeps everything else.
    let added = set(&before, &key("ui.bar.height_px"), &json!(40)).unwrap();
    assert_golden_text!("set_new_key", added);
    let section = set(
        &before,
        &key("ai.providers.anthropic"),
        &json!({"kind": "anthropic"}),
    )
    .unwrap();
    assert_golden_text!("set_new_section", section);
    let removed = unset(&before, &key("ui.reduced_motion")).unwrap();
    assert_golden_text!("unset_reduced_motion", removed);

    // CRLF files stay CRLF, still changing only one line.
    let crlf = before.replace('\n', "\r\n");
    let after = set(&crlf, &key("ui.theme"), &json!("light")).unwrap();
    assert_eq!(after, theme.replace('\n', "\r\n"));
}
