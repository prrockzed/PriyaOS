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
    // An input method is still composing pre-edit text. (Tests set it: Qt
    // Quick Test can't inject pre-edit text.)
    property bool composing: area.inputMethodComposing || area.preeditText !== ""
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
                height: Math.min(area.implicitHeight, metrics.lineSpacing * root.maxLines + area.topPadding + area.bottomPadding)
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
                        if (enter && (event.modifiers & Qt.ShiftModifier)) {
                            // A plain newline (Qt would insert U+2028).
                            area.insert(area.cursorPosition, "\n");
                            event.accepted = true;
                        } else if (enter) {
                            // While composing, Enter belongs to the input
                            // method: never a send, never a stray newline.
                            if (!root.composing)
                                root.submit();
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Escape) {
                            root.escapePressed();
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Up && root.recallPrevious()) {
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
                anchors.bottomMargin: Theme.space4 + Theme.space2
                iconName: "arrow-up"
                accessibleName: qsTr("Send")
                variant: area.text.trim() === "" && !root.busy ? "ghost" : "primary"
                enabled: area.text.trim() !== "" || root.busy
                busy: root.busy
                activeFocusOnTab: false
                onClicked: root.submit()
            }

            FontMetrics {
                id: metrics

                font: area.font
            }
        }

        Text {
            id: hints

            visible: root.showHints
            text: qsTr("Enter send · Shift+Enter new line · ↑ last prompt · Esc close")
            font: Theme.caption
            color: Theme.textTertiary
            Accessible.ignored: true
        }
    }
}
