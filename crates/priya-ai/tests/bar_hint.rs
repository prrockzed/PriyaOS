//! The Priya bar's "Connect a model" screen shows the same commands as the
//! "no model yet" hint, so changing the suggested model (as when Gemini
//! 2.5 closed to new keys) can't leave the bar suggesting a dead one.

use priya_ai::gateway::route::{GEMINI_MODEL, LOCAL_MODEL};

// covers: UX-02, AI-21
// suite: unit
#[test]
fn the_bar_suggests_the_same_models_as_the_hint() {
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ui/PriyaUI/PriyaBarContent.qml"
    );
    let qml = std::fs::read_to_string(file).expect("the bar's content");
    for command in [
        format!("priya settings set ai.strategy fixed:gemini/{GEMINI_MODEL}"),
        format!("priya settings set ai.strategy fixed:local/{LOCAL_MODEL}"),
    ] {
        assert!(qml.contains(&command), "the bar should show `{command}`");
    }
}
