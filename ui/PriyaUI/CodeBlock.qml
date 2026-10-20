import QtQuick
import QtQuick.Controls.Basic as T

// A code block in an answer: JetBrains Mono, a language label, a Copy
// button and sideways scrolling instead of wrapping. (Highlighting comes in
// 5.4.)
Rectangle {
    id: root

    property string code
    property string language
    // Still arriving: a caret after the last character.
    property bool streaming: false

    implicitWidth: Theme.space48 * 10
    implicitHeight: bar.height + scroller.height
    radius: Theme.radiusControl
    color: Theme.bgSurface2
    border.width: 1
    border.color: Theme.borderSubtle
    clip: true

    function caretAt(position, width) {
        return codeText.positionToRectangle(position);
    }

    Item {
        id: bar

        width: parent.width
        height: copy.height + Theme.space8

        Text {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space12
            anchors.verticalCenter: parent.verticalCenter
            text: root.language !== "" ? root.language : qsTr("code")
            font: Theme.caption
            color: Theme.textSecondary
            Accessible.ignored: true
        }

        Button {
            id: copy

            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.verticalCenter: parent.verticalCenter
            size: "small"
            variant: "ghost"
            iconName: "copy"
            text: qsTr("Copy")
            Accessible.description: qsTr("Copies the code")
            onClicked: Clipboard.copy(root.code)
        }

        Rectangle {
            anchors.bottom: parent.bottom
            width: parent.width
            height: 1
            color: Theme.borderSubtle
        }
    }

    Flickable {
        id: scroller

        y: bar.height
        width: parent.width
        height: codeText.height
        contentWidth: codeText.width
        contentHeight: codeText.height
        flickableDirection: Flickable.HorizontalFlick
        boundsBehavior: Flickable.StopAtBounds
        clip: true

        T.ScrollBar.horizontal: T.ScrollBar {
            policy: codeText.width > scroller.width ? T.ScrollBar.AsNeeded : T.ScrollBar.AlwaysOff
        }

        TextEdit {
            id: codeText

            width: Math.max(implicitWidth, scroller.width)
            leftPadding: Theme.space12
            rightPadding: Theme.space12
            topPadding: Theme.space8 + Theme.space2
            bottomPadding: Theme.space8 + Theme.space2
            text: root.code
            readOnly: true
            selectByMouse: true
            wrapMode: TextEdit.NoWrap
            textFormat: TextEdit.PlainText
            font: Qt.font({
                family: Theme.fontMono,
                pixelSize: Theme.small.pixelSize
            })
            color: Theme.textPrimary
            selectionColor: Theme.accentSubtle
            Accessible.role: Accessible.StaticText
            Accessible.name: root.language !== "" ? qsTr("%1 code").arg(root.language) : qsTr("Code")
            Accessible.description: text

            Rectangle {
                // Width is an argument so a resize moves the caret too.
                readonly property rect end: root.caretAt(codeText.length, codeText.width)

                visible: root.streaming
                x: end.x + 1
                y: end.y
                width: Math.max(1, Theme.scale * 1.5)
                height: end.height
                color: Theme.accent
            }
        }
    }
}
