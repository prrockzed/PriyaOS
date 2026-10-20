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

    TestCase {
        name: "PriyaBarContent"
        when: windowShown

        function init() {
            bar.phase = "idle";
            bar.question = "";
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
    }
}
