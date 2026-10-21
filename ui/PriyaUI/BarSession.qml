import QtQuick
import "api/Api.js" as Api

// What the Priya bar does (0.13, UX-02): asks priyad through a PriyaClient
// and shows the answer in a PriyaBarContent. Not visual and free of
// Quickshell, so it is tested with a mock transport; the shell's PriyaBar
// gives it the client and the view.
QtObject {
    id: session

    property var client
    property var view
    // A new conversation starts after this long without a question.
    property int idleMs: 10 * 60 * 1000
    // The clock, replaceable in tests.
    property var now: () => Date.now()

    // Follow-ups continue this conversation; empty starts a new one.
    property string conversationId
    // The question being answered, and its intent once priyad has named it.
    property string asked
    property string intentId
    property real lastAsked: 0
    // ui.bar.width_px, followed live like the theme; the shell sizes the
    // bar with it.
    property int barWidth: 720

    // Esc or click outside when nothing is being answered.
    signal closeRequested

    function submit(text) {
        const question = text.trim();
        if (question === "" || view.answering || view.phase === "unavailable")
            return;
        if (lastAsked > 0 && now() - lastAsked > idleMs)
            conversationId = "";
        lastAsked = now();
        asked = question;
        internal.stopWanted = false;
        internal.submittedAt = now();
        view.question = question;
        view.answer = "";
        view.model = "";
        view.local = false;
        view.providerLabel = "";
        view.error = null;
        view.phase = "thinking";
        view.input.text = "";
        const context = {
            "surface": "bar"
        };
        if (conversationId !== "")
            context.conversation_id = conversationId;
        client.request(Api.methods.intentSubmit, {
            "text": question,
            "context": context
        }, result => {
            conversationId = result.conversation_id;
            intentId = result.intent_id;
            client.watch(intentId, step => session.step(step));
            // Esc came before priyad named the intent.
            if (internal.stopWanted)
                stop();
        }, error => session.failed(error));
    }

    // Esc or Stop while answering. The answer so far stays, marked stopped,
    // once priyad says it has stopped.
    function stop() {
        if (intentId === "") {
            internal.stopWanted = view.phase === "thinking";
            return;
        }
        client.request(Api.methods.intentCancel, {
            "intent_id": intentId
        });
    }

    function retry() {
        submit(asked);
    }

    // Ctrl+N: the next question starts a new conversation.
    function newConversation() {
        if (view.answering || view.phase === "unavailable")
            return;
        conversationId = "";
        asked = "";
        view.question = "";
        view.answer = "";
        view.error = null;
        view.phase = "idle";
        view.input.text = "";
    }

    function step(step) {
        switch (step.kind) {
        case "started":
            view.model = step.model || "";
            view.local = step.local === true;
            view.providerLabel = label(step.provider || "");
            break;
        case "delta":
            if (view.phase === "thinking")
                console.debug(internal.perf, "submit → first delta:", now() - internal.submittedAt, "ms");
            view.answer += step.text || "";
            view.phase = "streaming";
            break;
        case "done":
            intentId = "";
            if (step.text !== undefined)
                view.answer = step.text;
            view.phase = "done";
            break;
        case "error":
            failed(step.error || {});
            break;
        case "cancelled":
            intentId = "";
            if (view.answer === "") {
                // Nothing arrived: back to the question, to ask again.
                view.phase = "idle";
                view.input.text = asked;
            } else {
                view.phase = "stopped";
            }
            break;
        }
    }

    // Asking failed: say why, and keep the question in the input so Enter
    // asks again.
    function failed(error) {
        intentId = "";
        internal.stopWanted = false;
        view.error = error;
        view.input.text = asked;
        if (client.state === "unavailable") {
            view.phase = "unavailable";
        } else if (client.state === "connecting") {
            // A blip: the question waits in the input until it's back.
            view.phase = "idle";
        } else if (error.kind === "not_found") {
            // While answering, only "no model chosen" is not_found.
            view.phase = "noModel";
        } else {
            view.phase = "failed";
            // priyad names no model before a network failure: ask which
            // one answers, to tell offline apart from a broken setting.
            if (error.kind === "unavailable" && view.providerLabel === "")
                client.request(Api.methods.aiStatus, {}, status => {
                    if (view.phase === "failed" && status.configured && status.provider) {
                        view.local = status.local === true;
                        view.providerLabel = label(status.provider);
                    }
                });
        }
    }

    // "gemini" → "Gemini", for "Priya can't reach Gemini".
    function label(provider) {
        return provider.charAt(0).toUpperCase() + provider.slice(1);
    }

    function connectionChanged() {
        if (client.state === "connected") {
            if (view.phase === "unavailable")
                view.phase = "idle";
            followTheme();
            return;
        }
        // priyad stops an answer with the connection that asked for it:
        // keep what arrived, and the question to ask again.
        if (intentId !== "" || view.answering) {
            intentId = "";
            internal.stopWanted = false;
            if (view.answer === "") {
                view.phase = "idle";
                view.input.text = asked;
            } else {
                view.phase = "stopped";
            }
        }
        if (client.state === "unavailable")
            view.phase = "unavailable";
    }

    // The bar follows ui.theme (DS-02) and ui.bar.width_px live: read on
    // connect, then every setting.changed. The subscription ends with the
    // connection.
    function followTheme() {
        client.request(Api.methods.settingsGet, {
            "key": "ui.theme"
        }, result => session.applyTheme(result.value));
        client.request(Api.methods.settingsGet, {
            "key": "ui.bar.width_px"
        }, result => session.applyWidth(result.value));
        client.request(Api.methods.eventsSubscribe, {
            "types": ["setting.changed"]
        }, result => client.watch(result.subscription, params => {
                // params.event is the stored event; its envelope is inside.
                const event = params.event && params.event.event;
                if (!event || event.type !== "setting.changed")
                    return;
                if (event.payload.key === "ui.bar.width_px") {
                    if (event.payload.new === null || event.payload.new === undefined)
                        // Unset: ask what the default is.
                        client.request(Api.methods.settingsGet, {
                            "key": "ui.bar.width_px"
                        }, read => session.applyWidth(read.value));
                    else
                        session.applyWidth(event.payload.new);
                    return;
                }
                if (event.payload.key !== "ui.theme")
                    return;
                if (event.payload.new === null || event.payload.new === undefined)
                    // Unset: the default is back, so ask what it is.
                    client.request(Api.methods.settingsGet, {
                        "key": "ui.theme"
                    }, read => session.applyTheme(read.value));
                else
                    session.applyTheme(event.payload.new);
            }));
    }

    function applyWidth(value) {
        if (typeof value === "number" && value > 0)
            barWidth = value;
    }

    function applyTheme(value) {
        if (value === "light" || value === "dark")
            Theme.mode = value;
    }

    // A session made after the client connected reads the theme now.
    Component.onCompleted: {
        if (client && client.state === "connected")
            followTheme();
    }

    // Not part of the API.
    property QtObject internals: QtObject {
        id: internal

        property bool stopWanted: false
        property real submittedAt: 0
        property LoggingCategory perf: LoggingCategory {
            name: "priya.bar.perf"
            defaultLogLevel: LoggingCategory.Warning
        }

        property Connections fromView: Connections {
            target: session.view

            function onSubmitted(text) {
                session.submit(text);
            }

            function onStopRequested() {
                session.stop();
            }

            function onRetryRequested() {
                session.retry();
            }

            function onNewConversationRequested() {
                session.newConversation();
            }

            function onCloseRequested() {
                session.closeRequested();
            }
        }

        property Connections fromClient: Connections {
            target: session.client

            function onStateChanged() {
                session.connectionChanged();
            }
        }
    }
}
