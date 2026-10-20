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
            id: providerName

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
            providerName.text = "";
            providerName.errorText = "";
            Theme.scale = 1;
            Theme.reducedMotion = true;
        }

        // covers: DS-03, DS-06
        // suite: ui
        function test_promptinput_hints_are_its_description() {
            prompt.showHints = true;
            compare(prompt.editor.Accessible.description, "Enter send · Shift+Enter new line · ↑ last prompt · Esc close");
            prompt.showHints = false;
            compare(prompt.editor.Accessible.description, "");
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
            prompt.history = ["older"];
            prompt.busy = true;
            keyClick(Qt.Key_Return);
            keyClick(Qt.Key_Return, Qt.ShiftModifier);
            keyClick(Qt.Key_Up);
            compare(sent.count, 1, "not while busy");
            compare(prompt.text, "wait", "busy keys change nothing");
            keyClick(Qt.Key_Escape);
            compare(escaped.count, 2, "Esc still works while busy");
            prompt.busy = false;
            prompt.text = "abcdef";
            prompt.editor.select(1, 4);
            keyClick(Qt.Key_Return, Qt.ShiftModifier);
            compare(prompt.text, "a\nef", "Shift+Enter replaces the selection");
        }

        // covers: DS-03
        // suite: ui
        // acceptance: 0.11
        function test_promptinput_no_submit_during_ime_preedit() {
            composingPrompt.editor.forceActiveFocus(Qt.TabFocusReason);
            composingPrompt.text = "नमस्ते";
            // Pre-edit text can't be injected from Qt Quick Test.
            composingPrompt.simulateComposing = true;
            keyClick(Qt.Key_Return);
            verify(!composingPrompt.submit());
            compare(composedSent.count, 0, "Enter belongs to the input method");
            compare(composingPrompt.text, "नमस्ते", "text kept");
            composingPrompt.simulateComposing = false;
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
            // Heights settle after the layout pass, so wait for each.
            const settled = text => {
                prompt.text = text;
                wait(100);
                return prompt.height;
            };
            const one = settled("one");
            const three = settled("1\n2\n3");
            const six = settled("1\n2\n3\n4\n5\n6");
            verify(three > one, "grows");
            verify(six > three, "grows to 6 lines");
            verify(!prompt.overflowing);
            const nine = settled("1\n2\n3\n4\n5\n6\n7\n8\n9");
            compare(nine, six, "stops growing after 6 lines");
            verify(prompt.overflowing, "then scrolls");
            compare(prompt.placeholderText, "Ask or tell Priya anything…");
        }

        // covers: DS-03, DS-06
        // suite: ui
        function test_textfield_error_is_shown_and_read_out() {
            compare(providerName.editor.Accessible.name, "Provider name");
            providerName.errorText = qsTr("Use lowercase letters, digits, - and _.");
            compare(providerName.editor.Accessible.description, "Use lowercase letters, digits, - and _.");
            providerName.editor.forceActiveFocus(Qt.TabFocusReason);
            keyClick(Qt.Key_A);
            compare(providerName.text, "a");
            compare(audit.problems(scene), []);
        }
    }
}
