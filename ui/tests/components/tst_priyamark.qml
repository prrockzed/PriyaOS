// PriyaMark (0.13). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI

TestCase {
    id: test

    name: "PriyaMark"

    Component {
        id: markComponent

        PriyaMark {}
    }

    // Colours change at once, without the 200 ms ease.
    function init() {
        Theme.reducedMotion = true;
        Theme.mode = "light";
    }

    function cleanup() {
        Theme.scale = 1;
        Theme.reducedMotion = false;
    }

    // covers: UX-02, DS-02
    // suite: ui
    function test_priyamark_tint_follows_mood_and_theme() {
        const mark = createTemporaryObject(markComponent, test);
        compare(mark.tint, Theme.accent);
        mark.mood = "quiet";
        compare(mark.tint, Theme.textTertiary);
        mark.mood = "warning";
        compare(mark.tint, Theme.warning);
        mark.mood = "error";
        compare(mark.tint, Theme.danger);
        // A copy: a colour read from a property follows it.
        const light = String(mark.tint);
        Theme.mode = "dark";
        verify(!Qt.colorEqual(mark.tint, light), "the tint changes with the theme");
        compare(mark.tint, Theme.danger);
    }

    // covers: UX-02, DS-06
    // suite: ui
    function test_priyamark_scales_and_is_named() {
        const mark = createTemporaryObject(markComponent, test);
        compare(mark.width, 28);
        Theme.scale = 2;
        compare(mark.width, 56);
        // A wider item keeps the drawing square, in the middle.
        mark.width = 100;
        const drawing = mark.children[0];
        compare(drawing.width, 56);
        compare(drawing.x, 22);
        compare(mark.Accessible.name, "Priya");
        compare(mark.Accessible.role, Accessible.Graphic);
    }
}
