pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Effects
import "ErrorMessages.js" as ErrorMessages

// What the Priya bar shows (0.13, UX-02; mockup ui/mockups/0.13/bar.html):
// the mark and the input on top, the result below, key hints at the foot.
// It has no window, so the gallery and tests show every state; the shell
// puts it in a layer-shell panel and drives it from PriyaClient. It only
// turns keys into signals: what they do is the shell's job.
FocusScope {
    id: root

    // "idle", "thinking", "streaming", "done", "stopped"; or, when there is
    // no answer, "failed" (see `error`), "noModel" or "unavailable"
    // (priyad can't be reached).
    property string phase: "idle"
    // The question being answered, shown above the answer.
    property string question
    property string answer
    property string model
    property bool local: false
    // Why asking failed: priyad's error data {kind, retryable,
    // user_message, detail}.
    property var error: null
    // The cloud provider asked ("Gemini"), for "Priya can't reach Gemini".
    property string providerLabel
    // Seconds until the next try while priyad is unavailable; 0 says nothing.
    property int retrySeconds: 0
    // The tallest the bar may be (70 % of the screen); the result scrolls.
    property real maxHeight: Theme.space48 * 14
    readonly property alias input: prompt
    readonly property bool answering: phase === "thinking" || phase === "streaming"
    readonly property bool hasAnswer: phase === "streaming" || phase === "done" || phase === "stopped"
    // A cloud model that can't be reached: the local one may still answer.
    // (Settings that point nowhere fail the same way, but before any model
    // is named.)
    readonly property bool offline: phase === "failed" && error !== null && error.kind === "unavailable" && !local && providerLabel !== ""

    signal submitted(string text)
    signal stopRequested
    signal closeRequested
    signal newConversationRequested
    signal retryRequested

    function copyAnswer() {
        if (hasAnswer)
            answerCard.copyAnswer();
    }

    implicitWidth: Theme.space48 * 15
    implicitHeight: Math.min(column.implicitHeight, maxHeight)
    Accessible.role: Accessible.Dialog
    Accessible.name: qsTr("Priya bar")

    // Screen readers hear the answer once, when it is complete (Qt 6.8+).
    // and a failure as soon as it happens, since focus stays in the input.
    onPhaseChanged: {
        if (typeof Accessible.announce !== "function")
            return;
        if (phase === "done")
            Accessible.announce(answerCard.Accessible.description);
        else if (phase === "failed")
            Accessible.announce(failure.title + ". " + failure.body);
        else if (phase === "noModel")
            Accessible.announce(qsTr("Connect a model"));
        else if (phase === "unavailable")
            Accessible.announce(qsTr("Priya's background service isn't running"));
    }

    // Esc from anywhere in the bar; the input sends its own.
    Keys.onEscapePressed: root.pressEscape()
    Keys.onPressed: event => {
        const ctrl = event.modifiers & Qt.ControlModifier;
        if (ctrl && event.key === Qt.Key_N) {
            root.newConversationRequested();
            event.accepted = true;
        } else if (ctrl && (event.modifiers & Qt.ShiftModifier) && event.key === Qt.Key_C) {
            root.copyAnswer();
            event.accepted = true;
        }
    }

    function pressEscape() {
        if (answering)
            stopRequested();
        else
            closeRequested();
    }

    RectangularShadow {
        anchors.fill: surface
        radius: Theme.radiusPanel
        offset.y: Theme.elevationOverlayShadow1Y
        blur: Theme.elevationOverlayShadow1Blur
        color: Theme.elevationOverlayShadow1
    }

    RectangularShadow {
        anchors.fill: surface
        radius: Theme.radiusPanel
        offset.y: Theme.elevationOverlayShadow2Y
        blur: Theme.elevationOverlayShadow2Blur
        color: Theme.elevationOverlayShadow2
    }

    Rectangle {
        id: surface

        anchors.fill: parent
        radius: Theme.radiusPanel
        color: Theme.bgBase
        border.width: 1
        border.color: Theme.elevationOverlayBorder
    }

    Column {
        id: column

        width: root.width

        Row {
            id: topRow

            x: Theme.space12 + Theme.space2
            topPadding: Theme.space12
            bottomPadding: Theme.space12
            spacing: Theme.space12

            PriyaMark {
                // Lined up with the input's first line.
                y: Theme.space12 + Theme.space2
                mood: root.offline ? "warning" : root.phase === "failed" || root.phase === "unavailable" ? "error" : root.phase === "stopped" || root.phase === "noModel" ? "quiet" : "ready"
            }

            PromptInput {
                id: prompt

                width: root.width - x - Theme.space12 - topRow.x
                focus: true
                // Nothing to ask until priyad is back.
                enabled: root.phase !== "unavailable"
                placeholderText: root.question !== "" ? qsTr("Ask a follow-up…") : qsTr("Ask Priya anything…")
                accessibleName: qsTr("Ask Priya")
                accessibleDescription: mainHints.text
                onSubmitted: text => root.submitted(text)
                onEscapePressed: root.pressEscape()
            }
        }

        Flickable {
            id: result

            width: root.width
            height: Math.min(contentHeight, root.maxHeight - topRow.height - foot.height)
            // Nothing to show while idle: then it takes no room.
            visible: root.phase !== "idle"
            contentHeight: results.height
            interactive: contentHeight > height
            clip: true
            boundsBehavior: Flickable.StopAtBounds

            Column {
                id: results

                x: Theme.space12
                width: parent.width - 2 * x
                bottomPadding: Theme.space12
                spacing: Theme.space8

                Row {
                    id: askedRow

                    visible: root.question !== "" && (root.answering || root.hasAnswer)
                    x: Theme.space4
                    width: parent.width - 2 * x
                    spacing: Theme.space8

                    Text {
                        id: asked

                        textFormat: Text.PlainText
                        text: qsTr("You asked")
                        font: Theme.small
                        color: Theme.textSecondary
                        Accessible.ignored: true
                    }

                    Text {
                        width: parent.width - asked.width - parent.spacing
                        textFormat: Text.PlainText
                        text: root.question
                        wrapMode: Text.Wrap
                        font: Theme.small
                        color: Theme.textSecondary
                        Accessible.role: Accessible.StaticText
                        Accessible.name: qsTr("You asked: %1").arg(root.question)
                    }
                }

                Card {
                    visible: root.phase === "thinking"
                    width: parent.width

                    Skeleton {
                        width: parent.width
                    }
                }

                AnswerCard {
                    id: answerCard

                    visible: root.hasAnswer
                    width: parent.width
                    text: root.answer
                    model: root.model
                    local: root.local
                    streaming: root.phase === "streaming"
                    stopped: root.phase === "stopped"
                    // A long answer scrolls inside the card, so its footer
                    // (Copy) stays in view; Theme.space48 + space32 is the
                    // card's padding and footer.
                    maxBodyHeight: Math.max(Theme.space48 * 2, root.maxHeight - topRow.height - foot.height - (askedRow.visible ? askedRow.height + results.spacing : 0) - results.bottomPadding - Theme.space48 - Theme.space32)
                    onStopRequested: root.stopRequested()
                }

                // Asking failed; the question stays in the input to ask again.
                Card {
                    visible: root.phase === "failed"
                    width: parent.width

                    ErrorState {
                        id: failure

                        readonly property var known: root.error ? ErrorMessages.forKind(root.error.kind) : null
                        // Each failure starts with its details closed.
                        readonly property var shown: root.error

                        onShownChanged: detailsOpen = false
                        width: parent.width
                        iconName: root.offline ? "cloud-off" : "triangle-alert"
                        title: root.offline ? qsTr("Priya can't reach %1").arg(root.providerLabel) : root.error && root.error.user_message ? root.error.user_message.replace(/\.$/, "") : known ? known.title : qsTr("Something went wrong")
                        body: root.offline ? qsTr("Your internet connection seems to be off. Try again when you're back online, or use the model on this laptop:") : known ? known.body : ""
                        // Not for what won't change by asking again (a missing key).
                        showRetry: !root.error || root.error.retryable !== false
                        details: root.error ? ["kind: " + root.error.kind].concat(root.error.detail ? [root.error.detail] : []).join("\n") : ""
                        onRetryRequested: root.retryRequested()

                        CodeBlock {
                            visible: root.offline
                            width: parent.width
                            language: "bash"
                            code: "priya settings set ai.strategy fixed:local/qwen2.5:0.5b"
                        }
                    }
                }

                Card {
                    visible: root.phase === "noModel"
                    width: parent.width

                    EmptyState {
                        width: parent.width
                        iconName: "sparkles"
                        title: qsTr("Connect a model")
                        body: qsTr("Priya needs a model to answer. Run one of these in a terminal, then ask again.")

                        Text {
                            width: parent.width
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            text: qsTr("A free cloud model (Gemini). The first line asks for your key.")
                            font: Theme.caption
                            color: Theme.textTertiary
                        }

                        CodeBlock {
                            width: parent.width
                            language: "bash"
                            code: "priya ai key set gemini\npriya settings set ai.strategy fixed:gemini/gemini-2.5-flash"
                        }

                        Text {
                            width: parent.width
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            text: qsTr("Or a model on this laptop (Ollama).")
                            font: Theme.caption
                            color: Theme.textTertiary
                        }

                        CodeBlock {
                            width: parent.width
                            language: "bash"
                            code: "priya settings set ai.strategy fixed:local/qwen2.5:0.5b"
                        }
                    }
                }

                Card {
                    visible: root.phase === "unavailable"
                    width: parent.width

                    ErrorState {
                        width: parent.width
                        iconName: "triangle-alert"
                        showRetry: false
                        title: qsTr("Priya's background service isn't running")
                        body: qsTr("Start it in a terminal. The bar connects again by itself.")

                        CodeBlock {
                            width: parent.width
                            language: "bash"
                            code: "systemctl --user start priyad"
                        }

                        Text {
                            visible: root.retrySeconds > 0
                            width: parent.width
                            textFormat: Text.PlainText
                            text: qsTr("Trying again in %1 s…").arg(root.retrySeconds)
                            font: Theme.caption
                            color: Theme.textTertiary
                        }
                    }
                }
            }
        }

        Rectangle {
            id: foot

            width: root.width
            height: hints.height + 2 * Theme.space8
            color: "transparent"

            Rectangle {
                width: parent.width
                height: 1
                color: Theme.borderSubtle
            }

            // Wraps when the hints don't fit on one line.
            Flow {
                id: hints

                x: Theme.space16
                y: Theme.space8
                width: parent.width - 2 * x

                KeyHints {
                    id: mainHints

                    variant: "footer"
                    hints: root.phase === "unavailable" ? [
                        {
                            "keys": [qsTr("Esc")],
                            "label": qsTr("close")
                        }
                    ] : root.answering ? [
                        {
                            "keys": [qsTr("Esc")],
                            "label": qsTr("stop")
                        }
                    ] : root.hasAnswer ? [
                        {
                            "keys": [qsTr("Enter")],
                            "label": qsTr("ask")
                        },
                        {
                            "keys": [qsTr("Tab")],
                            "label": qsTr("Copy")
                        },
                        {
                            "keys": [qsTr("Esc")],
                            "label": qsTr("close")
                        }
                    ] : [
                        {
                            "keys": [qsTr("Enter")],
                            "label": qsTr("ask")
                        },
                        {
                            "keys": [qsTr("Shift"), qsTr("Enter")],
                            "label": qsTr("new line")
                        },
                        {
                            "keys": [qsTr("Esc")],
                            "label": qsTr("close")
                        }
                    ]
                }

                Item {
                    width: Math.max(0, hints.width - mainHints.width - conversationHints.width - 1)
                    height: 1
                }

                KeyHints {
                    id: conversationHints

                    objectName: "conversationHints"
                    variant: "footer"
                    // Once there is an answer to follow up on, not while it streams.
                    hints: root.question !== "" && root.hasAnswer && !root.answering ? [
                        {
                            "keys": [qsTr("Ctrl"), qsTr("N")],
                            "label": qsTr("new conversation")
                        }
                    ] : []
                }
            }
        }
    }
}
