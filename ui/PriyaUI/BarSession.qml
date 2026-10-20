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
        if (view.answering)
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
        view.error = error;
        view.phase = "failed";
        view.input.text = asked;
    }

    // "gemini" → "Gemini", for "Priya can't reach Gemini".
    function label(provider) {
        return provider.charAt(0).toUpperCase() + provider.slice(1);
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
    }
}
