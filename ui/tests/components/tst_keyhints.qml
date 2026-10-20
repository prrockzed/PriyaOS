// KeyHints (0.13). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI

Item {
    id: scene

    width: 480
    height: 200

    KeyHints {
        id: inline

        hints: [
            {
                "keys": ["Enter"],
                "label": "ask"
            },
            {
                "keys": ["Ctrl", "Shift", "C"],
                "label": "copy"
            }
        ]
    }

    KeyHints {
        id: footer

        y: 40
        variant: "footer"
        hints: inline.hints
    }

    TestCase {
        name: "KeyHints"
        when: windowShown

        // Visible items of `item` whose text or objectName is `what`.
        function shown(item, what) {
            let found = [];
            for (let i = 0; i < item.children.length; i++) {
                const child = item.children[i];
                if (!child.visible)
                    continue;
                if (child.objectName === what || child.text === what)
                    found.push(child);
                found = found.concat(shown(child, what));
            }
            return found;
        }

        // covers: DS-03, DS-06, UX-02
        // suite: ui
        function test_keyhints_draw_each_key_and_read_as_one_line() {
            compare(shown(inline, "cap").length, 4, "one cap per key");
            compare(shown(inline, "·").length, 1, "a dot between hints");
            compare(shown(inline, "+").length, 0);
            compare(inline.text, "Enter ask · Ctrl+Shift+C copy");
            compare(inline.Accessible.ignored, true);

            // The bar's footer: keys joined by "+", hints apart by a gap.
            compare(shown(footer, "cap").length, 4);
            compare(shown(footer, "+").length, 2);
            compare(shown(footer, "·").length, 0);
            verify(footer.spacing > inline.spacing);
            compare(footer.text, inline.text);
        }

        // covers: DS-03
        // suite: ui
        function test_keyhints_without_hints_say_nothing() {
            const none = createTemporaryObject(Qt.createComponent("../../PriyaUI/KeyHints.qml"), scene);
            compare(none.text, "");
            compare(none.width, 0);
        }
    }
}
