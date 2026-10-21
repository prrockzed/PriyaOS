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

        // Every test starts on a fresh connection, with nothing pending.
        function init() {
            fakeTransport.reset();
            fakeTransport.drop();
            tryCompare(barClient, "state", "connected");
            session.intentId = "";
            barView.phase = "idle";
            barView.local = false;
            barView.providerLabel = "";
            barView.model = "";
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
                "model": "gemini-3.8-flash",
                "provider": "gemini",
                "local": false
            });
            compare(barView.phase, "thinking", "thinking until the first words");
            compare(barView.model, "gemini-3.8-flash");
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
                "model": "gemini-3.8-flash"
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

        // covers: UX-02
        // suite: ui
        // acceptance: 0.13
        function test_bar_error_try_again_resubmits() {
            ask("what's using my disk?");
            const id = accept();
            stream(id, 1, "started", {
                "model": "gemini-3.8-flash",
                "provider": "gemini",
                "local": false
            });
            stream(id, 2, "error", {
                "error": {
                    "kind": "provider_error",
                    "retryable": true,
                    "user_message": "Gemini is busy right now."
                }
            });
            compare(barView.phase, "failed");
            compare(barView.error.user_message, "Gemini is busy right now.");
            compare(barView.input.text, "what's using my disk?", "the question stays to ask again");
            fakeTransport.sent = [];
            barView.retryRequested();
            compare(fakeTransport.lastRequest("v1.intent.submit").params.text, "what's using my disk?");
            compare(barView.phase, "thinking");

            // Offline: priyad names no model before a network failure, so
            // the bar asks which one answers.
            stream(accept(), 1, "error", {
                "error": {
                    "kind": "unavailable",
                    "retryable": true,
                    "user_message": "Priya can't reach Gemini."
                }
            });
            verify(!barView.offline, "not yet known");
            const status = fakeTransport.lastRequest("v1.ai.status");
            fakeTransport.reply(status.id, {
                "configured": true,
                "provider": "gemini",
                "model": "gemini-3.8-flash",
                "local": false
            });
            verify(barView.offline);
            compare(barView.providerLabel, "Gemini");
        }

        // covers: UX-02
        // suite: ui
        function test_bar_a_blip_mid_answer_keeps_what_arrived_and_asking_works() {
            ask("write a story");
            const id = accept();
            stream(id, 1, "started", {
                "model": "m",
                "provider": "local",
                "local": true
            });
            stream(id, 2, "delta", {
                "text": "Once"
            });
            fakeTransport.drop();
            compare(barView.phase, "stopped", "priyad stopped it with the connection");
            compare(barView.answer, "Once");
            tryCompare(barClient, "state", "connected");
            ask("again");
            compare(fakeTransport.lastRequest("v1.intent.submit").params.text, "again");

            // Before priyad even replied: the question waits in the input.
            fakeTransport.drop();
            tryCompare(barClient, "state", "connected");
            compare(barView.phase, "idle");
            compare(barView.input.text, "again");
        }

        // covers: UX-02
        // suite: ui
        // acceptance: 0.13
        function test_bar_no_model_shows_connect_empty_state() {
            ask("hi");
            stream(accept(), 1, "error", {
                "error": {
                    "kind": "not_found",
                    "retryable": false,
                    "user_message": "No model is chosen yet. Store a key with `priya ai key set gemini`, then set ai.strategy."
                }
            });
            compare(barView.phase, "noModel");
            compare(barView.input.text, "hi");
        }

        // covers: UX-02
        // suite: ui
        // acceptance: 0.13
        function test_bar_daemon_unavailable_then_reconnects() {
            ask("hi");
            accept();
            fakeTransport.accepting = false;
            fakeTransport.drop();
            tryCompare(barClient, "state", "unavailable", 2000);
            compare(barView.phase, "unavailable", "the answer went with the connection");
            verify(!barView.input.enabled, "nothing to ask until priyad is back");
            fakeTransport.accepting = true;
            tryCompare(barClient, "state", "connected", 3000);
            compare(barView.phase, "idle");
            verify(barView.input.enabled);
            ask("hi again");
            compare(fakeTransport.lastRequest("v1.intent.submit").params.text, "hi again");
        }

        // covers: DS-02
        // suite: ui
        // acceptance: 0.13
        function test_bar_theme_follows_setting_changed() {
            Theme.mode = "dark";
            fakeTransport.accepting = true;
            fakeTransport.drop();
            tryCompare(barClient, "state", "connected");
            // The latest connection's, newest first.
            const gets = fakeTransport.sent.filter(m => m.method === "v1.settings.get").reverse();
            const get = gets.find(m => m.params.key === "ui.theme");
            const width = gets.find(m => m.params.key === "ui.bar.width_px");
            fakeTransport.reply(width.id, {
                "value": 960
            });
            compare(session.barWidth, 960, "the bar's width is read on connect too");
            fakeTransport.reply(get.id, {
                "value": "light"
            });
            compare(Theme.mode, "light", "read on connect");
            const subscribe = fakeTransport.lastRequest("v1.events.subscribe");
            compare(subscribe.params.types, ["setting.changed"]);
            fakeTransport.reply(subscribe.id, {
                "subscription": "sub-theme"
            });
            // As priyad sends it: a stored event around the envelope.
            let seq = 0;
            const changed = (key, value) => fakeTransport.notify("v1.events.event", {
                    "subscription": "sub-theme",
                    "event": {
                        "seq": ++seq,
                        "prev_hash": "00",
                        "hash": "11",
                        "event": {
                            "id": "01JTHEME0000000000000000" + seq,
                            "ts": "2026-10-21T00:00:00Z",
                            "type": "setting.changed",
                            "version": 1,
                            "stream": "settings",
                            "actor": "user",
                            "payload": {
                                "key": key,
                                "old": null,
                                "new": value,
                                "source": "user"
                            }
                        }
                    }
                });
            changed("ui.theme", "dark");
            compare(Theme.mode, "dark", "follows a change live");
            changed("ui.bar.width_px", 800);
            compare(session.barWidth, 800, "and followed live");
            changed("ui.bar.width_px", null);
            const widthAgain = fakeTransport.lastRequest("v1.settings.get");
            compare(widthAgain.params.key, "ui.bar.width_px", "unset: the default is read");
            fakeTransport.reply(widthAgain.id, {
                "value": 720
            });
            compare(session.barWidth, 720);
            changed("ai.strategy", "light");
            compare(Theme.mode, "dark", "other settings don't touch it");
            changed("ui.theme", "sepia");
            compare(Theme.mode, "dark", "values it doesn't know are ignored");
            // Unset: read again, to get the default.
            changed("ui.theme", null);
            const again = fakeTransport.lastRequest("v1.settings.get");
            verify(again.id !== get.id);
            fakeTransport.reply(again.id, {
                "value": "light"
            });
            compare(Theme.mode, "light");
            Theme.mode = "dark";
        }
    }
}
