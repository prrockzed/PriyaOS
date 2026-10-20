// Card, Chip, Skeleton, EmptyState and ErrorState (0.11). Run with
// `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI
import ".."

Item {
    id: scene

    width: 1000
    height: 700

    Column {
        x: 20
        y: 20
        spacing: 12

        Row {
            spacing: 8

            Chip {
                id: python

                text: qsTr("Python")
            }

            Chip {
                id: sandbox

                mode: "sandbox"
            }

            Chip {
                id: cheapest

                selectable: true
                text: qsTr("Cheapest")
            }

            Chip {
                id: best

                selectable: true
                text: qsTr("Best")
            }
        }

        Card {
            id: card

            width: 560
            title: qsTr("Install Docker")
            iconName: "package"
            footer: Row {
                Button {
                    text: qsTr("Do it")
                    variant: "primary"
                }
            }

            Text {
                text: qsTr("I'll install Docker.")
                font: Theme.body
                color: Theme.textPrimary
            }
        }

        Skeleton {
            id: skeleton

            width: 420
        }

        Row {
            spacing: 12

            EmptyState {
                id: empty

                width: 420
                title: qsTr("No tasks yet")
                body: qsTr("Ask Priya to do something and it shows up here.")
                actionText: qsTr("Open the Priya bar")
            }

            ErrorState {
                id: error

                width: 420
                title: qsTr("Priya can't reach the model")
                body: qsTr("Your internet connection seems to be off. Connect, then try again.")
                details: "kind: unavailable\nprovider: anthropic\nrequest: 01JABCDEFGHJKMNPQRSTVWXYZ0"
            }
        }
    }

    Component {
        id: withCommand

        ErrorState {
            width: 420
            title: qsTr("Priya's background service isn't running")
            body: qsTr("Start it in a terminal.")

            CodeBlock {
                width: parent.width
                language: "bash"
                code: "systemctl --user start priyad"
            }
        }
    }

    Component {
        id: emptyWithCommand

        EmptyState {
            width: 420
            title: qsTr("Connect a model")
            body: qsTr("Run this in a terminal.")

            CodeBlock {
                width: parent.width
                language: "bash"
                code: "priya ai key set gemini"
            }
        }
    }

    Component {
        id: emptyHidden

        EmptyState {
            width: 420
            title: qsTr("Connect a model")
            body: qsTr("Run this in a terminal.")

            CodeBlock {
                visible: false
                width: parent.width
                code: "hidden"
            }
        }
    }

    Component {
        id: emptyPlain

        EmptyState {
            width: 420
            title: qsTr("Connect a model")
            body: qsTr("Run this in a terminal.")
        }
    }

    TextEdit {
        id: pasteTarget

        visible: false
    }

    SignalSpy {
        id: emptyAction

        target: empty
        signalName: "actionTriggered"
    }

    SignalSpy {
        id: retries

        target: error
        signalName: "retryRequested"
    }

    A11yAudit {
        id: audit
    }

    TestCase {
        name: "Surfaces"
        when: windowShown

        function init() {
            Theme.reducedMotion = false;
            Theme.scale = 1;
            Theme.mode = "dark";
            error.detailsOpen = false;
            cheapest.selected = false;
        }

        // covers: DS-06
        // suite: ui
        // acceptance: 0.11
        function test_skeleton_static_with_reduced_motion() {
            verify(skeleton.animating, "shimmers normally");
            Theme.reducedMotion = true;
            verify(!skeleton.animating, "still with reduced motion");
            compare(skeleton.Accessible.name, "Thinking…");
        }

        // covers: DS-03
        // suite: ui
        // acceptance: 0.11
        function test_errorstate_details_expand_and_copy() {
            const toggle = findButton(error, "Details");
            verify(toggle !== null, "has Details");
            const panelText = findDetails(error);
            verify(!panelText.visible || !panelText.parent.visible, "details start closed");
            mouseClick(toggle);
            verify(error.detailsOpen);
            verify(panelText.parent.visible, "details open");
            compare(toggle.text, "Hide details");
            // Columns lay out when polished: wait before clicking inside.
            waitForRendering(error);
            const copy = findButton(error, "Copy");
            mouseClick(copy);
            pasteTarget.text = "";
            pasteTarget.paste();
            compare(pasteTarget.text, error.details, "the details are on the clipboard");
            mouseClick(findButton(error, "Try again"));
            compare(retries.count, 1);
            mouseClick(toggle);
            verify(!error.detailsOpen);
        }

        // covers: UX-02, DS-03
        // suite: ui
        function test_states_hold_commands_under_the_body() {
            for (const component of [withCommand, emptyWithCommand]) {
                const state = createTemporaryObject(component, scene);
                waitForRendering(state);
                const code = state.content[0];
                const body = findChild(state, "body");
                const gap = code.mapToItem(state, 0, 0).y - body.mapToItem(state, 0, body.height).y;
                compare(gap, Theme.space8 + Theme.space4 + Theme.space2, "the mockup's 14 px under the body");
                compare(code.width, state.width);
                const retry = findButton(state, "Try again");
                if (retry)
                    verify(code.mapToItem(state, 0, code.height).y < retry.mapToItem(state, 0, 0).y, "commands come before the buttons");
            }
            // Nothing shown takes no room, even a hidden child.
            const plain = createTemporaryObject(emptyPlain, scene);
            const hidden = createTemporaryObject(emptyHidden, scene);
            waitForRendering(hidden);
            compare(hidden.implicitHeight, plain.implicitHeight);
        }

        // covers: DS-03, DS-06
        // suite: ui
        function test_chips_show_mode_words_and_toggle_from_keyboard() {
            compare(sandbox.text, "Sandbox", "a word, not colour alone");
            compare(sandbox.shownIcon, "box");
            compare(sandbox.tone, Theme.modeSandbox);
            verify(!python.activeFocusOnTab, "static chips aren't focusable");
            compare(cheapest.Accessible.role, Accessible.CheckBox);
            cheapest.forceActiveFocus(Qt.TabFocusReason);
            keyClick(Qt.Key_Space);
            verify(cheapest.selected);
            compare(cheapest.shownIcon, "check");
            keyClick(Qt.Key_Return);
            verify(!cheapest.selected);
            mouseClick(best);
            verify(best.selected);
            best.selected = false;
        }

        // covers: DS-03, DS-06
        // suite: ui
        function test_cards_and_states_pass_the_audit() {
            verify(card.hasHeader);
            verify(card.height > 100);
            mouseClick(findButton(empty, "Open the Priya bar"));
            compare(emptyAction.count, 1);
            compare(audit.problems(scene), []);
        }

        function findButton(root, label) {
            const stack = [root];
            while (stack.length) {
                const item = stack.pop();
                if (item.text === label && item.clicked !== undefined)
                    return item;
                for (let i = 0; i < item.children.length; ++i)
                    stack.push(item.children[i]);
            }
            return null;
        }

        function findDetails(root) {
            const stack = [root];
            while (stack.length) {
                const item = stack.pop();
                if (item.Accessible.name === "Technical details")
                    return item;
                for (let i = 0; i < item.children.length; ++i)
                    stack.push(item.children[i]);
            }
            return null;
        }
    }
}
