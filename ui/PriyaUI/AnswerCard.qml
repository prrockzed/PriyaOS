pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as T
import "MarkdownBlocks.js" as MarkdownBlocks

// An answer from a model, rendered as it streams (11 §2.4). The Markdown is
// split into blocks (MarkdownBlocks.js) and only blocks that changed are
// re-rendered, so finished blocks never move; the height grows smoothly.
// Prose is Markdown text whose links open only on click; code goes in a
// CodeBlock. States: streaming (a caret), done, error (the text so far is
// kept, with an inline error), stopped (kept, with a Stopped chip), long
// (scrolls inside), any language; a local model's answer says so.
Card {
    id: root

    property string text
    property bool streaming: false
    // Empty unless the answer stopped with an error.
    property string errorText
    // Stopped before its end (Esc or Stop): what arrived stays, marked so.
    property bool stopped: false
    // The model runs on this computer; the footer says so, so it is always
    // clear whether a question left the machine.
    property bool local: false
    // The model that answered, e.g. "claude-sonnet-5-5".
    property string model
    // Taller answers scroll inside the card.
    property real maxBodyHeight: Theme.space48 * 9
    readonly property alias blockCount: blocks.count
    // Opens clicked http(s) and mailto links in the default app. Other
    // schemes (file:, javascript:…) only emit linkActivated.
    property bool openLinks: true

    signal stopRequested
    signal retryRequested
    signal detailsRequested
    signal linkActivated(string link)

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Answer")
    // The answer without Markdown marks, for screen readers.
    Accessible.description: text.replace(/```[^\n]*\n?|[*_`#>]|!?\[([^\]]*)\]\([^)]*\)/g, (match, label) => label ?? "")

    onTextChanged: sync()
    Component.onCompleted: sync()

    // Updates only the blocks that differ, so finished ones stay as they are.
    function sync() {
        const next = MarkdownBlocks.split(text);
        for (let i = 0; i < next.length; ++i) {
            if (i >= blocks.count) {
                blocks.append(next[i]);
                continue;
            }
            const now = blocks.get(i);
            if (now.kind !== next[i].kind || now.source !== next[i].source || now.lang !== next[i].lang || now.level !== next[i].level || now.open !== next[i].open)
                blocks.set(i, next[i]);
        }
        if (blocks.count > next.length)
            blocks.remove(next.length, blocks.count - next.length);
    }

    // A link was clicked (never on hover).
    function activateLink(link) {
        if (openLinks && /^(https?:|mailto:)/i.test(link))
            Qt.openUrlExternally(link);
        linkActivated(link);
    }

    function copyAnswer() {
        Clipboard.copy(text);
    }

    ListModel {
        id: blocks
    }

    footer: Item {
        implicitWidth: root.width - Theme.space24
        implicitHeight: actions.implicitHeight

        Row {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.space4 + Theme.space2

            Chip {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.stopped && !root.streaming
                iconName: "square"
                text: qsTr("Stopped")
            }

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                visible: !root.stopped || root.streaming
                name: "sparkles"
                color: Theme.textTertiary
            }

            Text {
                textFormat: Text.PlainText
                anchors.verticalCenter: parent.verticalCenter
                text: root.streaming ? qsTr("%1 · answering…").arg(root.model) : root.local ? qsTr("%1 · on this laptop").arg(root.model) : root.model
                font: Theme.caption
                color: Theme.textTertiary
                Accessible.role: Accessible.StaticText
                Accessible.name: root.stopped ? qsTr("Stopped. Answered by %1").arg(root.model) : root.local ? qsTr("Answered by %1, on this laptop").arg(root.model) : qsTr("Answered by %1").arg(root.model)
            }
        }

        Row {
            id: actions

            anchors.right: parent.right
            spacing: Theme.space8

            Button {
                visible: root.streaming
                size: "small"
                variant: "ghost"
                iconName: "square"
                text: qsTr("Stop")
                onClicked: root.stopRequested()
            }

            Button {
                visible: !root.streaming && root.errorText !== ""
                size: "small"
                variant: "ghost"
                text: qsTr("Details")
                onClicked: root.detailsRequested()
            }

            Button {
                visible: !root.streaming && root.errorText !== ""
                size: "small"
                iconName: "refresh-cw"
                text: qsTr("Try again")
                onClicked: root.retryRequested()
            }

            Button {
                id: copyAnswerButton

                visible: !root.streaming && root.errorText === ""
                size: "small"
                variant: "ghost"
                iconName: "copy"
                text: qsTr("Copy answer")
                onClicked: root.copyAnswer()
            }
        }
    }

    Flickable {
        id: scroller

        width: parent.width
        height: Math.min(content.height, root.maxBodyHeight)
        contentWidth: width
        contentHeight: content.height
        boundsBehavior: Flickable.StopAtBounds
        clip: contentHeight > height
        interactive: contentHeight > height

        // While streaming, new text stays in view if the reader was at the end.
        property bool following: true

        onContentYChanged: following = atYEnd || contentHeight <= height
        onContentHeightChanged: {
            if (root.streaming && following && contentHeight > height)
                contentY = contentHeight - height;
        }

        Behavior on height {
            enabled: Theme.animationsEnabled

            NumberAnimation {
                duration: Theme.motionStandard
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Theme.easingStandard
            }
        }

        T.ScrollBar.vertical: T.ScrollBar {
            policy: scroller.interactive ? T.ScrollBar.AsNeeded : T.ScrollBar.AlwaysOff
        }

        Column {
            id: content

            width: scroller.width
            spacing: Theme.space8

            Repeater {
                id: renderer

                model: blocks

                delegate: Loader {
                    id: block

                    required property int index
                    // Through `model`: a `source` role would shadow
                    // Loader.source.
                    required property var model
                    readonly property string kind: model.kind
                    readonly property string markdown: model.source
                    readonly property string lang: model.lang
                    readonly property bool open: model.open
                    readonly property int level: model.level

                    readonly property bool last: index === blocks.count - 1

                    width: content.width
                    sourceComponent: kind === "code" ? codeBlock : prose

                    Component {
                        id: prose

                        TextEdit {
                            id: proseText

                            width: block.width
                            readOnly: true
                            selectByMouse: true
                            // Rendered by MarkdownBlocks.toHtml: links in the
                            // accent colour, and nothing in the answer can
                            // become markup that loads a resource.
                            textFormat: TextEdit.RichText
                            wrapMode: TextEdit.Wrap
                            text: MarkdownBlocks.toHtml({
                                "kind": block.kind,
                                "source": block.markdown
                            }, {
                                "link": String(Theme.accent),
                                "muted": String(Theme.textSecondary),
                                "mono": Theme.fontMono
                            })
                            // # and ## are titles, ### and below body-large.
                            font: block.kind !== "heading" ? Theme.body : Qt.font({
                                family: Theme.bodyLarge.family,
                                pixelSize: (block.level <= 2 ? Theme.title : Theme.bodyLarge).pixelSize,
                                weight: Font.DemiBold
                            })
                            color: Theme.textPrimary
                            selectionColor: Theme.accentSubtle
                            Accessible.role: block.kind === "heading" ? Accessible.Heading : Accessible.StaticText
                            Accessible.name: getText(0, length)
                            onLinkActivated: link => root.activateLink(link)

                            function caretAt(position, width, size) {
                                return positionToRectangle(position);
                            }

                            // Links are clickable, in the accent colour.
                            HoverHandler {
                                cursorShape: proseText.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.IBeamCursor
                            }

                            // The streaming caret after the last word.
                            Rectangle {
                                // Recomputed when the text, width or font changes.
                                readonly property rect end: proseText.caretAt(proseText.length, proseText.width, proseText.font.pixelSize)

                                visible: root.streaming && block.last
                                x: end.x + 1
                                y: end.y
                                width: Math.max(1, Theme.scale * 1.5)
                                height: end.height
                                color: Theme.accent

                                SequentialAnimation on opacity {
                                    running: root.streaming && block.last && Theme.animationsEnabled
                                    loops: Animation.Infinite

                                    PropertyAnimation {
                                        to: 0
                                        duration: 0
                                    }

                                    PauseAnimation {
                                        duration: 530
                                    }

                                    PropertyAnimation {
                                        to: 1
                                        duration: 0
                                    }

                                    PauseAnimation {
                                        duration: 530
                                    }
                                }
                            }
                        }
                    }

                    Component {
                        id: codeBlock

                        CodeBlock {
                            width: block.width
                            code: block.markdown
                            language: block.lang
                            streaming: root.streaming && block.last && block.open
                        }
                    }
                }
            }

            Rectangle {
                visible: root.errorText !== ""
                width: parent.width
                height: visible ? errorRow.implicitHeight + Theme.space16 : 0
                radius: Theme.radiusControl
                color: Qt.alpha(Theme.danger, 0.08)
                border.width: 1
                border.color: Theme.danger
                Accessible.role: Accessible.AlertMessage
                Accessible.name: root.errorText

                Row {
                    id: errorRow

                    x: Theme.space8 + Theme.space2
                    y: Theme.space8
                    width: parent.width - 2 * x
                    spacing: Theme.space8

                    Icon {
                        name: "triangle-alert"
                        color: Theme.danger
                    }

                    Text {
                        width: parent.width - Theme.space24
                        wrapMode: Text.Wrap
                        textFormat: Text.StyledText
                        // The error comes from outside: escaped, so it stays text.
                        text: qsTr("%1 <font color=\"%2\">What's above is kept.</font>").arg(MarkdownBlocks.escapeHtml(root.errorText)).arg(Theme.textSecondary)
                        font: Theme.small
                        color: Theme.danger
                        Accessible.ignored: true
                    }
                }
            }
        }
    }
}
