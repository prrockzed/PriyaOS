// PriyaBarContent (0.13). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI

Item {
    id: scene

    width: 800
    height: 900

    PriyaBarContent {
        id: bar

        width: 720
    }

    SignalSpy {
        id: submitted

        target: bar
        signalName: "submitted"
    }

    SignalSpy {
        id: stops

        target: bar
        signalName: "stopRequested"
    }

    SignalSpy {
        id: closes

        target: bar
        signalName: "closeRequested"
    }

    SignalSpy {
        id: fresh

        target: bar
        signalName: "newConversationRequested"
    }

    TextEdit {
        id: pasteTarget
    }

    TestCase {
        name: "PriyaBarContent"
        when: windowShown

        function init() {
            bar.phase = "idle";
            bar.question = "";
            bar.answer = "";
            bar.maxHeight = Theme.space48 * 14;
            bar.input.text = "";
            bar.input.editor.forceActiveFocus();
            for (const spy of [submitted, stops, closes, fresh])
                spy.clear();
        }

        // covers: UX-02, DS-06
        // suite: ui
        function test_priyabarcontent_keys_ask_stop_close_and_start_over() {
            compare(bar.input.editor.Accessible.name, "Ask Priya");
            keyClick("h");
            keyClick("i");
            keyClick(Qt.Key_Return);
            compare(submitted.count, 1);
            compare(submitted.signalArguments[0][0], "hi");

            bar.phase = "thinking";
            bar.question = "hi";
            keyClick(Qt.Key_Escape);
            compare(stops.count, 1, "Esc while thinking stops");
            bar.phase = "streaming";
            keyClick(Qt.Key_Escape);
            compare(stops.count, 2, "Esc while streaming stops");
            compare(closes.count, 0);

            bar.phase = "stopped";
            keyClick(Qt.Key_Escape);
            compare(closes.count, 1, "Esc when idle closes");
            compare(bar.input.placeholderText, "Ask a follow-up…");

            keyClick(Qt.Key_N, Qt.ControlModifier);
            compare(fresh.count, 1);
        }

        // covers: UX-02
        // suite: ui
        function test_priyabarcontent_copies_the_answer_with_ctrl_shift_c() {
            bar.phase = "done";
            bar.question = "what is btrfs?";
            bar.answer = "**btrfs** is a file system.";
            keyClick(Qt.Key_C, Qt.ControlModifier | Qt.ShiftModifier);
            pasteTarget.text = "";
            pasteTarget.paste();
            compare(pasteTarget.text, bar.answer);
        }

        // covers: UX-02, DS-06
        // suite: ui
        function test_priyabarcontent_idle_has_no_empty_result_area() {
            waitForRendering(bar);
            const idle = bar.implicitHeight;
            bar.question = "hi";
            bar.answer = "Hello.";
            bar.phase = "done";
            tryVerify(() => bar.implicitHeight > idle);
            bar.question = "";
            bar.answer = "";
            bar.phase = "idle";
            tryCompare(bar, "implicitHeight", idle, 1000, "back to the frame alone");
        }

        // covers: UX-02, DS-06
        // suite: ui
        function test_priyabarcontent_hints_follow_the_phase() {
            const said = () => bar.input.editor.Accessible.description;
            compare(said(), "Enter ask · Shift+Enter new line · Esc close");
            bar.question = "hi";
            bar.phase = "streaming";
            compare(said(), "Esc stop");
            bar.phase = "done";
            compare(said(), "Enter ask · Tab Copy · Esc close");
        }

        // covers: UX-02
        // suite: ui
        function test_priyabarcontent_a_long_answer_scrolls_inside_and_copy_stays() {
            bar.maxHeight = 480;
            bar.phase = "done";
            bar.question = "why?";
            bar.answer = Array(60).fill("A line of the answer.").join("\n\n");
            waitForRendering(bar);
            verify(bar.height <= 480, "the bar never grows past its maximum");
            const copy = findCopy(bar);
            verify(copy !== null);
            const bottom = copy.mapToItem(bar, 0, copy.height).y;
            verify(bottom <= bar.height, "Copy answer stays in view: " + bottom + " > " + bar.height);
        }

        // covers: UX-02, DS-06
        // suite: ui
        function test_priyabarcontent_tab_goes_from_the_input_through_the_answer() {
            bar.question = "free space?";
            bar.answer = "Run this:\n\n```bash\nsudo paccache -rk2\n```";
            bar.phase = "done";
            waitForRendering(bar);
            bar.input.editor.forceActiveFocus();
            keyClick(Qt.Key_Tab);
            compare(focusedText(), "Copy", "the code block first, in reading order");
            keyClick(Qt.Key_Tab);
            compare(focusedText(), "Copy answer");
            keyClick(Qt.Key_Escape);
            compare(closes.count, 1, "Esc from Copy closes");
            bar.answer = "Rivers";
            bar.phase = "streaming";
            waitForRendering(bar);
            bar.input.editor.forceActiveFocus();
            keyClick(Qt.Key_Tab);
            compare(focusedText(), "Stop", "while streaming: Stop");
        }

        // The label of the item with keyboard focus.
        function focusedText() {
            // Through an array: the focus item's type doesn't say it has text.
            const item = [bar.Window.activeFocusItem][0];
            return item ? String(item.text) : "";
        }

        function findCopy(root) {
            const stack = [root];
            while (stack.length) {
                const item = stack.pop();
                if (item.text === "Copy answer" && item.clicked !== undefined)
                    return item;
                for (let i = 0; i < item.children.length; ++i)
                    stack.push(item.children[i]);
            }
            return null;
        }
    }
}
