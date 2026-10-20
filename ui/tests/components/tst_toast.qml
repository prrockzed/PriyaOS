// Toast and ToastStack (0.11). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI
import ".."

Item {
    id: scene

    width: 480
    height: 600

    ToastStack {
        id: stack

        x: 20
        y: 20
        width: 380
        timeout: 600
    }

    SignalSpy {
        id: actions

        target: stack
        signalName: "actionTriggered"
    }

    A11yAudit {
        id: audit
    }

    TestCase {
        name: "Toast"
        when: windowShown

        function init() {
            while (stack.count > 0)
                stack.dismiss(0);
            actions.clear();
            mouseMove(scene, scene.width - 5, scene.height - 5);
        }

        function toasts() {
            const found = [];
            const stackItems = [stack];
            while (stackItems.length) {
                const item = stackItems.pop();
                if (item.Accessible.role === Accessible.AlertMessage && item.timeout !== undefined)
                    found.push(item);
                for (let i = 0; i < item.children.length; ++i)
                    stackItems.push(item.children[i]);
            }
            return found.sort((a, b) => a.y - b.y);
        }

        // covers: DS-03
        // suite: ui
        // acceptance: 0.11
        function test_toast_max_three_and_pause_on_hover() {
            for (let i = 1; i <= 5; ++i)
                stack.show({
                    "id": "t" + i,
                    "kind": i === 2 ? "error" : "success",
                    "title": "Toast " + i
                });
            compare(stack.count, 3, "at most 3 on screen");
            compare(stack.queued, 2);
            waitForRendering(stack);

            const first = toasts()[0];
            compare(first.title, "Toast 1");
            compare(first.Accessible.role, Accessible.AlertMessage, "announced");
            mouseMove(first, first.width / 2, first.height / 2);
            tryVerify(() => first.paused, 1000, "hover pauses");
            // Longer than the timeout: the others leave, the hovered one stays.
            wait(stack.timeout * 2);
            verify(toasts().some(t => t.title === "Toast 1"), "the hovered toast stays");
            verify(!toasts().some(t => t.title === "Toast 2"), "the others leave on time");
            mouseMove(scene, scene.width - 5, scene.height - 5);
            tryVerify(() => !toasts().some(t => t.title === "Toast 1"), stack.timeout * 3, "it leaves once the mouse does");
        }

        // covers: DS-03, DS-06
        // suite: ui
        function test_toast_focus_pauses_and_action_dismisses() {
            stack.show({
                "id": "undo",
                "kind": "success",
                "title": "Docker is installed",
                "actionText": "Undo"
            });
            waitForRendering(stack);
            const toast = toasts()[0];
            toast.forceActiveFocus(Qt.TabFocusReason);
            verify(toast.paused, "focus pauses");
            compare(audit.problems(scene), []);
            const undo = findLabelled(toast, "Undo");
            mouseClick(undo);
            compare(actions.count, 1);
            compare(actions.signalArguments[0][0], "undo");
            compare(stack.count, 0);
        }

        function findLabelled(root, label) {
            const stackItems = [root];
            while (stackItems.length) {
                const item = stackItems.pop();
                if (item.text === label && item.clicked !== undefined)
                    return item;
                for (let i = 0; i < item.children.length; ++i)
                    stackItems.push(item.children[i]);
            }
            return null;
        }
    }
}
