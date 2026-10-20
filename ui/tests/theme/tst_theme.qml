// The generated Theme singleton (0.10). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI

TestCase {
    name: "Theme"

    function init() {
        Theme.reducedMotion = true; // first: switch at once, so values are final
        Theme.mode = "dark";
        Theme.scale = 1;
    }

    // covers: DS-02
    // suite: ui
    // acceptance: 0.10
    // regression: 0.10
    function test_theme_singleton_switches_mode() {
        compare(Theme.mode, "dark");
        compare(String(Theme.bgBase), "#121214");
        compare(String(Theme.textPrimary), "#ededf0");

        Theme.mode = "light";
        compare(String(Theme.bgBase), "#fafaf7");
        compare(String(Theme.textPrimary), "#18181b");
        compare(String(Theme.accent), "#915500");

        Theme.mode = "oled";
        compare(String(Theme.bgBase), "#000000");
        compare(String(Theme.textPrimary), "#ededf0", "from dark");

        // A mode with no palette shows dark.
        Theme.mode = "sepia";
        compare(String(Theme.bgBase), "#121214");
    }

    // covers: DS-01
    // suite: ui
    function test_translucent_shadows_and_motion_values() {
        Theme.mode = "light";
        // #18181B at 6%: QML colours take alpha first.
        fuzzyCompare(Theme.elevationCardShadow1.a, 15 / 255, 0.001);
        fuzzyCompare(Theme.elevationCardShadow1.r, 0x18 / 255, 0.001);
        compare(Theme.elevationCardShadow2Blur, 12);
        Theme.mode = "dark";
        compare(Theme.elevationCardShadow1.a, 0);
        compare(String(Theme.elevationCardBorder), "#34343d");
        compare(Theme.easingStandard, [0.2, 0, 0, 1, 1, 1]);
        compare(Theme.radiusPill, 9999);
        compare(Theme.radiusCard, 10);
    }

    // covers: DS-02
    // suite: ui
    function test_colours_ease_between_modes_when_motion_is_on() {
        Theme.reducedMotion = false;
        Theme.mode = "light";
        verify(String(Theme.bgBase) !== "#fafaf7", "still easing");
        tryCompare(Theme, "bgBase", Qt.color("#fafaf7"), 2000);
    }

    // covers: DS-01
    // suite: ui
    function test_sizes_follow_scale_and_motion_follows_reduced_motion() {
        Theme.scale = 2;
        compare(Theme.space8, 16);
        compare(Theme.body.pixelSize, 28);
        Theme.scale = 1;
        compare(Theme.body.pixelSize, 14);
        compare(Theme.motionStandard, 0);
        Theme.reducedMotion = false;
        compare(Theme.motionStandard, 200);
        compare(Theme.body.family, "Inter");
        compare(Theme.title.weight, 600);
    }
}
