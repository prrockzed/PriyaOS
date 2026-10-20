// Button and IconButton (0.11). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI
import ".."

Item {
    id: scene

    width: 480
    height: 200

    Row {
        x: 20
        y: 60
        spacing: 12

        Button {
            id: keep

            text: qsTr("Keep changes")
            variant: "primary"
        }

        Button {
            id: undo

            text: qsTr("Undo")
            iconName: "undo-2"
        }

        IconButton {
            id: copy

            iconName: "copy"
            accessibleName: qsTr("Copy")
            variant: "ghost"
        }
    }

    SignalSpy {
        id: keepClicks

        target: keep
        signalName: "clicked"
    }

    SignalSpy {
        id: copyClicks

        target: copy
        signalName: "clicked"
    }

    A11yAudit {
        id: audit
    }

    TestCase {
        name: "Button"
        when: windowShown

        function init() {
            keep.enabled = true;
            keep.busy = false;
            keepClicks.clear();
            copyClicks.clear();
            Theme.scale = 1;
        }

        // covers: DS-03, DS-06
        // suite: ui
        // acceptance: 0.11
        function test_button_keyboard_activation_and_disabled() {
            keep.forceActiveFocus(Qt.TabFocusReason);
            keyClick(Qt.Key_Space);
            compare(keepClicks.count, 1, "Space");
            keyClick(Qt.Key_Return);
            tryCompare(keepClicks, "count", 2, 1000, "Return");
            keyClick(Qt.Key_Enter);
            tryCompare(keepClicks, "count", 3, 1000, "Enter");

            keep.busy = true;
            keyClick(Qt.Key_Space);
            keyClick(Qt.Key_Return);
            mouseClick(keep);
            wait(200);
            compare(keepClicks.count, 3, "busy ignores presses");
            compare(keep.Accessible.name, "Keep changes", "label kept while busy");

            keep.busy = false;
            keep.enabled = false;
            mouseClick(keep);
            compare(keepClicks.count, 3, "disabled ignores clicks");
            undo.forceActiveFocus(Qt.TabFocusReason);
            keyClick(Qt.Key_Backtab);
            verify(!keep.activeFocus, "Tab skips a disabled button");
            compare(keep.opacity, 0.45);
        }

        // covers: DS-06
        // suite: ui
        // acceptance: 0.11
        function test_iconbutton_exposes_accessible_name() {
            compare(copy.Accessible.name, "Copy");
            compare(copy.Accessible.role, Accessible.Button);
            compare(copy.width, copy.height, "square");
            copy.forceActiveFocus(Qt.TabFocusReason);
            keyClick(Qt.Key_Return);
            tryCompare(copyClicks, "count", 1);
            // The tooltip is the same name, after 600 ms.
            const tooltip = findChild(copy, "tooltip");
            verify(tooltip !== null, "has a tooltip");
            compare(tooltip.text, "Copy");
            compare(tooltip.delay, 600);
            wait(300);
            verify(!tooltip.visible, "not before 600 ms");
            tryCompare(tooltip, "visible", true, 2000);
            // No name, no button.
            let made = null;
            try {
                made = Qt.createQmlObject("import PriyaUI; IconButton { iconName: \"x\" }", scene);
            } catch (e) {
                made = null;
            }
            verify(made === null, "an IconButton without accessibleName can't be made");
        }

        // covers: DS-06
        // suite: ui
        function test_buttons_pass_the_audit_and_scale() {
            compare(audit.problems(scene), []);
            compare(keep.height, 36);
            Theme.scale = 2;
            compare(keep.height, 72);
        }
    }
}
