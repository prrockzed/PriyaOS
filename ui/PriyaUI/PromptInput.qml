import QtQuick
import QtQuick.Controls.Basic as T

// The multi-line prompt of the Priya bar. It grows from 1 to 6 lines, then
// scrolls. Enter sends, Shift+Enter adds a line, Esc emits escapePressed,
// and Up in an empty input brings back earlier prompts. Nothing is sent
// while an input method is still composing (Hindi, Japanese…).
FocusScope {
    id: root

    property alias text: area.text
    property string placeholderText: qsTr("Ask or tell Priya anything…")
    // Waiting for the answer: the text is kept, greyed and read-only.
    property bool busy: false
    // Earlier prompts, oldest first; submit() adds to it.
    property var history: []
    property bool showHints: false
    property string preview
    readonly property alias editor: area
    readonly property bool showFocus: area.activeFocus || preview === "focus"
    readonly property int maxLines: 6
    // More text than 6 lines show: the editor scrolls.
    readonly property bool overflowing: area.implicitHeight > scroller.height
    // Acts as if an input method were composing. Qt Quick Test can't inject
    // pre-edit text, so the tests use this.
    property bool simulateComposing: false
    // An input method is still composing pre-edit text.
    readonly property bool composing: area.inputMethodComposing || area.preeditText !== "" || simulateComposing
    // Which history entry Up showed last; -1 when none.
    property int recalled: -1

    signal submitted(string text)
    signal escapePressed

    implicitWidth: Theme.space48 * 11 + Theme.space32
    implicitHeight: column.implicitHeight

    // Sends the text unless it is empty or still being composed.
    function submit() {
        const value = area.text;
        if (busy || composing || value.trim() === "")
            return false;
        history = history.concat([value]);
        recalled = -1;
        area.clear();
        submitted(value);
        return true;
    }

    // Up in an empty input (or one showing a recalled prompt): one older.
    function recallPrevious() {
        if (history.length === 0)
            return false;
        if (area.text !== "" && (recalled < 0 || area.text !== history[recalled]))
            return false;
        recalled = recalled < 0 ? history.length - 1 : Math.max(0, recalled - 1);
        area.text = history[recalled];
        area.cursorPosition = area.length;
        return true;
    }

    Column {
        id: column

        width: root.width
        spacing: Theme.space4

        Rectangle {
            id: frame

            width: parent.width
            height: Math.max(send.implicitHeight, scroller.height) + Theme.space8 + Theme.space12
            radius: Theme.radiusCard
            color: Theme.bgSurface1
            border.width: root.showFocus ? 2 : 1
            border.color: root.showFocus ? Theme.accent : Theme.borderStrong

            Behavior on height {
                NumberAnimation {
                    duration: Theme.motionMicro
                }
            }

            T.ScrollView {
                id: scroller

                anchors.left: parent.left
                anchors.right: send.left
                anchors.bottom: parent.bottom
                anchors.leftMargin: Theme.space12 + Theme.space2
                anchors.rightMargin: Theme.space8
                anchors.bottomMargin: Theme.space8 + Theme.space2
                // The editor's own line height, so exactly 6 lines fit.
                height: Math.min(area.implicitHeight, area.contentHeight / Math.max(1, area.lineCount) * root.maxLines + area.topPadding + area.bottomPadding)
                T.ScrollBar.horizontal.policy: T.ScrollBar.AlwaysOff

                T.TextArea {
                    id: area

                    focus: true
                    activeFocusOnTab: true
                    readOnly: root.busy
                    wrapMode: TextEdit.Wrap
                    font: Theme.bodyLarge
                    color: root.busy ? Theme.textTertiary : Theme.textPrimary
                    placeholderText: root.placeholderText
                    placeholderTextColor: Theme.textTertiary
                    selectionColor: Theme.accentSubtle
                    selectedTextColor: Theme.textPrimary
                    topPadding: Theme.space4
                    bottomPadding: Theme.space4
                    leftPadding: 0
                    rightPadding: 0
                    background: null
                    Accessible.role: Accessible.EditableText
                    Accessible.name: root.placeholderText
                    Accessible.description: root.showHints ? hints.text : ""

                    Keys.onPressed: event => {
                        const enter = event.key === Qt.Key_Return || event.key === Qt.Key_Enter;
                        if (event.key === Qt.Key_Escape && !root.composing) {
                            root.escapePressed();
                            event.accepted = true;
                        } else if (root.busy) {
                            // Read-only while sending: no newline, no recall.
                            event.accepted = enter || event.key === Qt.Key_Up;
                        } else if (enter && (event.modifiers & Qt.ShiftModifier)) {
                            // A plain newline (Qt would insert U+2028),
                            // replacing any selection.
                            area.remove(area.selectionStart, area.selectionEnd);
                            area.insert(area.cursorPosition, "\n");
                            event.accepted = true;
                        } else if (enter) {
                            // While composing, Enter belongs to the input
                            // method: never a send, never a stray newline.
                            if (!root.composing)
                                root.submit();
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Up && !root.composing && root.recallPrevious()) {
                            event.accepted = true;
                        }
                    }
                }
            }

            IconButton {
                id: send

                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.rightMargin: Theme.space8
                anchors.bottomMargin: Theme.space8 + Theme.space2
                iconName: "arrow-up"
                accessibleName: qsTr("Send")
                variant: area.text.trim() === "" && !root.busy ? "ghost" : "primary"
                enabled: area.text.trim() !== "" || root.busy
                busy: root.busy
                activeFocusOnTab: false
                onClicked: root.submit()
            }
        }

        // Key hints: each key drawn as a small key cap.
        Row {
            id: hints

            readonly property string text: qsTr("Enter send · Shift+Enter new line · ↑ last prompt · Esc close")

            visible: root.showHints
            spacing: Theme.space4

            Repeater {
                model: [
                    {
                        "keys": [qsTr("Enter")],
                        "label": qsTr("send")
                    },
                    {
                        "keys": [qsTr("Shift"), qsTr("Enter")],
                        "label": qsTr("new line")
                    },
                    {
                        "keys": ["↑"],
                        "label": qsTr("last prompt")
                    },
                    {
                        "keys": [qsTr("Esc")],
                        "label": qsTr("close")
                    }
                ]

                delegate: Row {
                    id: hint

                    required property var modelData
                    required property int index

                    spacing: Theme.space4

                    Text {
                        visible: hint.index > 0
                        text: "·"
                        font: Theme.caption
                        color: Theme.textTertiary
                        Accessible.ignored: true
                    }

                    Repeater {
                        model: hint.modelData.keys

                        delegate: Rectangle {
                            id: cap

                            required property string modelData

                            width: capText.implicitWidth + Theme.space8
                            height: capText.implicitHeight + Theme.space2
                            radius: Theme.space4
                            color: "transparent"
                            border.width: 1
                            border.color: Theme.borderSubtle

                            Text {
                                id: capText

                                anchors.centerIn: parent
                                text: cap.modelData
                                font: Qt.font({
                                    family: Theme.fontMono,
                                    pixelSize: Theme.caption.pixelSize
                                })
                                color: Theme.textSecondary
                                Accessible.ignored: true
                            }
                        }
                    }

                    Text {
                        text: hint.modelData.label
                        font: Theme.caption
                        color: Theme.textTertiary
                        Accessible.ignored: true
                    }
                }
            }
        }
    }
}
