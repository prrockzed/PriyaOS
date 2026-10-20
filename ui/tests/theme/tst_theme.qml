// The generated Theme singleton (0.10). Run with:
// QT_QPA_PLATFORM=offscreen qmltestrunner -import ui -input ui/tests/theme
import QtQuick
import QtTest
import PriyaUI

TestCase {
    name: "Theme"

    function init() {
        Theme.mode = "dark"
        Theme.reducedMotion = true // switch at once, so values are final
    }

    // covers: DS-02
    // suite: ui
    // acceptance: 0.10
    function test_theme_singleton_switches_mode() {
        compare(Theme.mode, "dark")
        compare(String(Theme.bgBase), "#121214")
        compare(String(Theme.textPrimary), "#ededf0")

        Theme.mode = "light"
        compare(String(Theme.bgBase), "#fafaf7")
        compare(String(Theme.textPrimary), "#18181b")
        compare(String(Theme.accent), "#915500")

        // A mode with no palette shows dark.
        Theme.mode = "sepia"
        compare(String(Theme.bgBase), "#121214")
    }

    // covers: DS-01
    // suite: ui
    function test_sizes_follow_scale_and_motion_follows_reduced_motion() {
        Theme.scale = 2
        compare(Theme.space8, 16)
        compare(Theme.body.pixelSize, 28)
        Theme.scale = 1
        compare(Theme.body.pixelSize, 14)
        compare(Theme.motionStandard, 0)
        Theme.reducedMotion = false
        compare(Theme.motionStandard, 200)
        compare(Theme.body.family, "Inter")
        compare(Theme.title.weight, 600)
    }
}
