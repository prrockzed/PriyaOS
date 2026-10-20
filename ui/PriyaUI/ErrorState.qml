import QtQuick
import "ErrorMessages.js" as ErrorMessages

// Something went wrong: what happened (title) and what to do next (body),
// never blaming the user. "Try again", and "Details", which opens the
// technical text with a Copy button.
Item {
    id: root

    property string iconName: "cloud-off"
    property string title
    property string body
    // Technical text (error kind, request id, …); no Details when empty.
    property string details
    property bool showRetry: true
    property bool detailsOpen: false
    // More under the body, such as commands to copy (CodeBlock), each
    // child setting its own width.
    default property alias content: extra.data

    signal retryRequested

    // Shows a priyad error ({code, message, data: {kind, retryable,
    // user_message}}) in translated words (ErrorMessages.js).
    function showApiError(error) {
        const shown = ErrorMessages.describe(error);
        title = shown.title;
        body = shown.body;
        details = shown.details;
        showRetry = shown.retryable;
    }

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
            color: Theme.danger
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
            objectName: "body"
            text: root.body
            visible: text !== ""
            wrapMode: Text.Wrap
            font: Theme.body
            color: Theme.textSecondary
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        // What `content` holds, under the body, full width. With nothing
        // shown it has no height, and the column skips it.
        Item {
            width: parent.width
            height: extra.height > 0 ? extra.y + extra.height : 0

            Column {
                id: extra

                // The column's own spacing plus this: the mockup's 14 px.
                y: Theme.space8
                width: parent.width
                spacing: Theme.space8
            }
        }

        Item {
            width: 1
            height: Theme.space8 - parent.spacing
        }

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: Theme.space8

            Button {
                id: retry

                visible: root.showRetry
                variant: "primary"
                iconName: "refresh-cw"
                text: qsTr("Try again")
                onClicked: root.retryRequested()
            }

            Button {
                id: toggle

                visible: root.details !== ""
                variant: "ghost"
                text: root.detailsOpen ? qsTr("Hide details") : qsTr("Details")
                Accessible.expandable: true
                Accessible.expanded: root.detailsOpen
                onClicked: root.detailsOpen = !root.detailsOpen
            }
        }

        Item {
            width: 1
            height: Theme.space8 - parent.spacing
            visible: panel.visible
        }

        Rectangle {
            id: panel

            visible: root.detailsOpen && root.details !== ""
            width: parent.width
            height: visible ? bar.height + detailText.height + 1 : 0
            radius: Theme.radiusControl
            color: Theme.bgSurface2
            border.width: 1
            border.color: Theme.borderSubtle

            Item {
                id: bar

                width: parent.width
                height: copy.height + Theme.space8

                Text {
                    textFormat: Text.PlainText
                    anchors.left: parent.left
                    anchors.leftMargin: Theme.space12
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Details")
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
                    Accessible.description: qsTr("Copies the details")
                    onClicked: Clipboard.copy(root.details)
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: Theme.borderSubtle
                }
            }

            TextEdit {
                id: detailText

                y: bar.height
                width: parent.width
                leftPadding: Theme.space12
                rightPadding: Theme.space12
                topPadding: Theme.space8
                bottomPadding: Theme.space8
                text: root.details
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.WrapAtWordBoundaryOrAnywhere
                font: Qt.font({
                    family: Theme.fontMono,
                    pixelSize: Theme.caption.pixelSize
                })
                color: Theme.textSecondary
                selectionColor: Theme.accentSubtle
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Technical details")
                Accessible.description: text
            }
        }
    }
}
