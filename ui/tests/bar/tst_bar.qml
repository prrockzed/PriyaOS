// The Priya bar's behaviour: BarSession with PriyaBarContent, talking to a
// mock priyad through a mock transport (0.13). Run with `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI
import "../"

Item {
    id: scene

    width: 800
    height: 900

    MockTransport {
        id: fakeTransport
    }

    PriyaClient {
        id: barClient

        transport: fakeTransport
    }

    PriyaBarContent {
        id: barView

        width: 720
    }

    BarSession {
        id: session

        client: barClient
        view: barView
    }

    SignalSpy {
        id: closes

        target: session
        signalName: "closeRequested"
    }

    TestCase {
        name: "Bar"
        when: windowShown

        property int intents: 0

        function init() {
            fakeTransport.reset();
            if (barClient.state !== "connected")
                fakeTransport.drop();
            tryCompare(barClient, "state", "connected");
            // A test may end mid-answer.
            session.intentId = "";
            barView.phase = "idle";
            session.newConversation();
            session.conversationId = "";
            session.lastAsked = 0;
            session.now = () => Date.now();
            closes.clear();
            barView.input.editor.forceActiveFocus();
        }

        // Types a question and presses Enter.
        function ask(text) {
            barView.input.text = text;
            barView.input.editor.forceActiveFocus();
            keyClick(Qt.Key_Return);
        }

        // priyad's reply to the last submit; returns the intent id.
        function accept(conversation) {
            const submit = fakeTransport.lastRequest("v1.intent.submit");
            verify(submit !== null, "a question was sent");
            const id = "intent-" + (++intents);
            fakeTransport.reply(submit.id, {
                "intent_id": id,
                "conversation_id": conversation || "conv-1"
            });
            return id;
        }

        function stream(intent, seq, kind, extra) {
            fakeTransport.notify("v1.intent.stream", Object.assign({
                "intent_id": intent,
                "seq": seq,
                "kind": kind
            }, extra || {}));
        }

        // covers: UX-02
        // suite: ui
        // acceptance: 0.13
        function test_bar_enter_submits_and_shows_thinking() {
            ask("  what is btrfs?  ");
            const submit = fakeTransport.lastRequest("v1.intent.submit");
            compare(submit.params.text, "what is btrfs?");
            compare(submit.params.context.surface, "bar");
            verify(submit.params.context.conversation_id === undefined, "a new conversation");
            compare(barView.phase, "thinking");
            compare(barView.question, "what is btrfs?");
            compare(barView.input.text, "", "the input is free for a follow-up");

            // Enter again while answering asks nothing more.
            ask("again?");
            compare(fakeTransport.sent.filter(m => m.method === "v1.intent.submit").length, 1);
        }

        // covers: UX-02
        // suite: ui
        // acceptance: 0.13
        function test_bar_stream_deltas_render_in_order() {
            ask("what is btrfs?");
            const id = accept();
            stream(id, 1, "started", {
                "model": "gemini-2.5-flash",
                "provider": "gemini",
                "local": false
            });
            compare(barView.phase, "thinking", "thinking until the first words");
            compare(barView.model, "gemini-2.5-flash");
            stream(id, 2, "delta", {
                "text": "Btrfs is"
            });
            compare(barView.phase, "streaming");
            stream("someone-else", 1, "delta", {
                "text": " NOT MINE"
            });
            stream(id, 3, "delta", {
                "text": " a file system."
            });
            stream(id, 4, "usage", {
                "usage": {
                    "input": 10,
                    "output": 4
                }
            });
            compare(barView.answer, "Btrfs is a file system.");
            stream(id, 5, "done", {
                "text": "Btrfs is a file system.",
                "model": "gemini-2.5-flash"
            });
            compare(barView.phase, "done");
            compare(barView.answer, "Btrfs is a file system.");

            // A follow-up continues the conversation.
            ask("who made it?");
            compare(fakeTransport.lastRequest("v1.intent.submit").params.context.conversation_id, "conv-1");
        }

        // covers: UX-02
        // suite: ui
        // acceptance: 0.13
        function test_bar_esc_cancels_then_closes() {
            ask("write a long essay");
            const id = accept();
            stream(id, 1, "started", {
                "model": "m",
                "provider": "local",
                "local": true
            });
            stream(id, 2, "delta", {
                "text": "Rivers"
            });
            keyClick(Qt.Key_Escape);
            const cancel = fakeTransport.lastRequest("v1.intent.cancel");
            compare(cancel.params.intent_id, id);
            compare(closes.count, 0, "the first Esc only stops");
            fakeTransport.reply(cancel.id, {});
            stream(id, 3, "cancelled");
            compare(barView.phase, "stopped");
            compare(barView.answer, "Rivers", "the partial answer stays");
            keyClick(Qt.Key_Escape);
            compare(closes.count, 1, "the second Esc closes");
        }

        // covers: UX-02
        // suite: ui
        function test_bar_esc_before_priyad_names_the_intent_still_stops() {
            ask("hi");
            keyClick(Qt.Key_Escape);
            compare(fakeTransport.lastRequest("v1.intent.cancel"), null, "no intent to name yet");
            const id = accept();
            compare(fakeTransport.lastRequest("v1.intent.cancel").params.intent_id, id);
            stream(id, 1, "cancelled");
            compare(barView.phase, "idle", "nothing arrived: back to the question");
            compare(barView.input.text, "hi");
        }

        // covers: UX-02
        // suite: ui
        function test_bar_ctrl_n_or_ten_idle_minutes_start_a_new_conversation() {
            let clock = 1000000;
            session.now = () => clock;
            ask("one");
            stream(accept("conv-a"), 1, "done", {
                "text": "1"
            });
            ask("two");
            compare(fakeTransport.lastRequest("v1.intent.submit").params.context.conversation_id, "conv-a");
            stream(accept("conv-a"), 1, "done", {
                "text": "2"
            });
            clock += 10 * 60 * 1000 + 1;
            ask("three");
            verify(fakeTransport.lastRequest("v1.intent.submit").params.context.conversation_id === undefined, "10 minutes idle: new");
            stream(accept("conv-b"), 1, "done", {
                "text": "3"
            });
            keyClick(Qt.Key_N, Qt.ControlModifier);
            compare(barView.phase, "idle");
            compare(barView.question, "");
            ask("four");
            verify(fakeTransport.lastRequest("v1.intent.submit").params.context.conversation_id === undefined, "Ctrl+N: new");
        }
    }
}
