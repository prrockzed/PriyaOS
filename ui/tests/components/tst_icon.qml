// Icon and FocusRing (0.11). Run with `cargo xtask test ui`.
import QtQuick
import QtQuick.Controls.Basic as T
import QtTest
import PriyaUI

Item {
    id: scene

    width: 400
    height: 200

    Icon {
        id: icon

        name: "copy"
    }

    T.Button {
        id: first

        x: 20
        y: 60
        width: 80
        height: 36
        Accessible.name: "first"

        FocusRing {
            id: firstRing
        }
    }

    T.Button {
        id: second

        x: 120
        y: 60
        width: 80
        height: 36
        Accessible.name: "second"

        FocusRing {
            id: secondRing
        }
    }

    Repeater {
        id: every

        // Every vendored icon (ui/PriyaUI/icons/README.md).
        model: ["arrow-up", "box", "check", "cloud-off", "copy", "eye", "folder", "history", "inbox", "info", "package", "refresh-cw", "sparkles", "square", "triangle-alert", "undo-2", "x", "zap"]

        delegate: Image {
            required property string modelData

            source: Qt.resolvedUrl("../../PriyaUI/icons/" + modelData + ".svg")
            sourceSize: Qt.size(16, 16)
        }
    }

    TestCase {
        name: "Icon"
        when: windowShown

        function init() {
            Theme.scale = 1;
            Theme.mode = "dark";
            icon.size = 16;
        }

        // covers: DS-03
        // suite: ui
        function test_icon_sizes_follow_scale_and_tint_follows_theme() {
            compare(icon.implicitWidth, 16);
            icon.size = 24;
            compare(icon.implicitHeight, 24);
            Theme.scale = 2;
            compare(icon.implicitWidth, 48);
            Theme.reducedMotion = true;
            compare(icon.color, Theme.textPrimary);
            Theme.mode = "light";
            compare(icon.color, Theme.textPrimary, "follows the mode");
            verify(icon.Accessible.ignored, "decoration only");
        }

        // covers: DS-03
        // suite: ui
        function test_every_vendored_icon_loads() {
            compare(every.count, 18);
            for (let i = 0; i < every.count; ++i)
                tryCompare(every.itemAt(i), "status", Image.Ready);
        }

        // covers: DS-06
        // suite: ui
        function test_focus_ring_shows_for_keyboard_focus_only() {
            // Focus as a click gives it (macOS buttons don't take focus on
            // click at all).
            first.forceActiveFocus(Qt.MouseFocusReason);
            verify(first.activeFocus);
            verify(!firstRing.visible, "no ring after a click");
            keyClick(Qt.Key_Tab);
            verify(second.activeFocus);
            verify(secondRing.visible, "ring after Tab");
            verify(!firstRing.visible);
            compare(secondRing.border.width, Theme.focusWidth);
            compare(secondRing.border.color, Theme.focus);
            compare(secondRing.width, second.width + 4 * Theme.focusWidth);
        }
    }
}
