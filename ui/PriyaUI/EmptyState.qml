import QtQuick

// What a screen shows with nothing in it yet: an icon, a title, a line of
// body text and an optional action.
Item {
    id: root

    property string iconName: "inbox"
    property string title
    property string body
    // The action's label; no button when empty.
    property string actionText

    signal actionTriggered

    implicitHeight: column.implicitHeight
    implicitWidth: Theme.space48 * 8 + Theme.space32 + Theme.space4

    Column {
        id: column

        width: root.width
        topPadding: Theme.space24
        bottomPadding: Theme.space24
        spacing: Theme.space4 + Theme.space2

        Icon {
            anchors.horizontalCenter: parent.horizontalCenter
            name: root.iconName
            size: 24
            color: Theme.textTertiary
        }

        Item {
            width: 1
            height: Theme.space4 - parent.spacing + Theme.space2
        }

        Text {
            textFormat: Text.PlainText
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: root.title
            wrapMode: Text.Wrap
            font: Qt.font({
                family: Theme.bodyLarge.family,
                pixelSize: Theme.bodyLarge.pixelSize,
                weight: Font.DemiBold
            })
            color: Theme.textPrimary
            Accessible.role: Accessible.Heading
            Accessible.name: text
        }

        Text {
            textFormat: Text.PlainText
            anchors.horizontalCenter: parent.horizontalCenter
            width: Math.min(parent.width, Theme.space48 * 6 + Theme.space24)
            horizontalAlignment: Text.AlignHCenter
            text: root.body
            visible: text !== ""
            wrapMode: Text.Wrap
            font: Theme.body
            color: Theme.textSecondary
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        Item {
            width: 1
            height: Theme.space8 - parent.spacing
            visible: root.actionText !== ""
        }

        Button {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: root.actionText !== ""
            text: root.actionText
            onClicked: root.actionTriggered()
        }
    }
}
