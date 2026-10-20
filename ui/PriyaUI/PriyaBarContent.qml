pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Effects

// What the Priya bar shows (0.13, UX-02; mockup ui/mockups/0.13/bar.html):
// the mark and the input on top, the result below, key hints at the foot.
// It has no window, so the gallery and tests show every state; the shell
// puts it in a layer-shell panel and drives it from PriyaClient. It only
// turns keys into signals: what they do is the shell's job.
FocusScope {
    id: root

    // "idle", "thinking", "streaming", "done" or "stopped".
    property string phase: "idle"
    // The question being answered, shown above the answer.
    property string question
    property string answer
    property string model
    property bool local: false
    // The tallest the bar may be (70 % of the screen); the result scrolls.
    property real maxHeight: Theme.space48 * 14
    readonly property alias input: prompt
    readonly property bool answering: phase === "thinking" || phase === "streaming"
    readonly property bool hasAnswer: phase === "streaming" || phase === "done" || phase === "stopped"

    signal submitted(string text)
    signal stopRequested
    signal closeRequested
    signal newConversationRequested

    function copyAnswer() {
        if (hasAnswer)
            answerCard.copyAnswer();
    }

    implicitWidth: Theme.space48 * 15
    implicitHeight: Math.min(column.implicitHeight, maxHeight)
    Accessible.role: Accessible.Dialog
    Accessible.name: qsTr("Priya bar")

    // Screen readers hear the answer once, when it is complete (Qt 6.8+).
    onPhaseChanged: {
        if (phase === "done" && typeof Accessible.announce === "function")
            Accessible.announce(answerCard.Accessible.description);
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
                mood: root.phase === "stopped" ? "quiet" : "ready"
            }

            PromptInput {
                id: prompt

                width: root.width - x - Theme.space12 - topRow.x
                focus: true
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

                    visible: root.question !== "" && root.phase !== "idle"
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
                    hints: root.answering ? [
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

                    variant: "footer"
                    hints: root.question !== "" && !root.answering ? [
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
