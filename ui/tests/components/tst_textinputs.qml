// TextField and PromptInput (0.11). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI
import ".."

Item {
    id: scene

    width: 640
    height: 420

    Column {
        x: 20
        y: 20
        spacing: 16

        TextField {
            id: name

            width: 360
            label: qsTr("Provider name")
            placeholderText: qsTr("For example, ollama")
        }

        PromptInput {
            id: prompt

            width: 560
        }

        PromptInput {
            id: composingPrompt

            width: 560
        }
    }

    SignalSpy {
        id: sent

        target: prompt
        signalName: "submitted"
    }

    SignalSpy {
        id: escaped

        target: prompt
        signalName: "escapePressed"
    }

    SignalSpy {
        id: composedSent

        target: composingPrompt
        signalName: "submitted"
    }

    A11yAudit {
        id: audit
    }

    TestCase {
        name: "TextInputs"
        when: windowShown

        function init() {
            prompt.text = "";
            prompt.history = [];
            prompt.recalled = -1;
            prompt.busy = false;
            sent.clear();
            escaped.clear();
            name.text = "";
            name.errorText = "";
            Theme.scale = 1;
            Theme.reducedMotion = true;
        }

        // covers: DS-03
        // suite: ui
        // acceptance: 0.11
        function test_promptinput_enter_submits_shift_enter_newline() {
            prompt.editor.forceActiveFocus(Qt.TabFocusReason);
            keyClick(Qt.Key_H);
            keyClick(Qt.Key_I);
            keyClick(Qt.Key_Return, Qt.ShiftModifier);
            keyClick(Qt.Key_X);
            compare(prompt.text, "hi\nx", "Shift+Enter adds a line");
            compare(sent.count, 0);
            keyClick(Qt.Key_Return);
            compare(sent.count, 1);
            compare(sent.signalArguments[0][0], "hi\nx");
            compare(prompt.text, "", "cleared after sending");
            keyClick(Qt.Key_Enter);
            compare(sent.count, 1, "nothing to send");
            keyClick(Qt.Key_Escape);
            compare(escaped.count, 1);
            prompt.text = "wait";
            prompt.busy = true;
            keyClick(Qt.Key_Return);
            compare(sent.count, 1, "not while busy");
        }

        // covers: DS-03
        // suite: ui
        // acceptance: 0.11
        function test_promptinput_no_submit_during_ime_preedit() {
            composingPrompt.editor.forceActiveFocus(Qt.TabFocusReason);
            composingPrompt.text = "नमस्ते";
            // Pre-edit text can't be injected from Qt Quick Test.
            composingPrompt.composing = true;
            keyClick(Qt.Key_Return);
            verify(!composingPrompt.submit());
            compare(composedSent.count, 0, "Enter belongs to the input method");
            compare(composingPrompt.text, "नमस्ते", "text kept");
            composingPrompt.composing = false;
            keyClick(Qt.Key_Return);
            compare(composedSent.count, 1, "sent once composing ends");
            compare(composedSent.signalArguments[0][0], "नमस्ते");
        }

        // covers: DS-03
        // suite: ui
        // acceptance: 0.11
        function test_promptinput_up_recalls_previous() {
            prompt.editor.forceActiveFocus(Qt.TabFocusReason);
            prompt.text = "first";
            keyClick(Qt.Key_Return);
            prompt.text = "second";
            keyClick(Qt.Key_Return);
            compare(prompt.text, "");
            keyClick(Qt.Key_Up);
            compare(prompt.text, "second");
            keyClick(Qt.Key_Up);
            compare(prompt.text, "first", "Up again goes further back");
            keyClick(Qt.Key_End);
            keyClick(Qt.Key_S);
            keyClick(Qt.Key_Up);
            compare(prompt.text, "firsts", "Up in edited text just moves the cursor");
        }

        // covers: DS-03
        // suite: ui
        function test_promptinput_grows_to_six_lines_then_scrolls() {
            prompt.text = "one";
            const one = prompt.height;
            prompt.text = "1\n2\n3";
            tryVerify(() => prompt.height > one);
            prompt.text = "1\n2\n3\n4\n5\n6";
            const six = prompt.height;
            prompt.text = "1\n2\n3\n4\n5\n6\n7\n8\n9";
            compare(prompt.height, six, "stops growing after 6 lines");
            compare(prompt.placeholderText, "Ask or tell Priya anything…");
        }

        // covers: DS-03, DS-06
        // suite: ui
        function test_textfield_error_is_shown_and_read_out() {
            compare(name.editor.Accessible.name, "Provider name");
            name.errorText = qsTr("Use lowercase letters, digits, - and _.");
            compare(name.editor.Accessible.description, "Use lowercase letters, digits, - and _.");
            name.editor.forceActiveFocus(Qt.TabFocusReason);
            keyClick(Qt.Key_A);
            compare(name.text, "a");
            compare(audit.problems(scene), []);
        }
    }
}
