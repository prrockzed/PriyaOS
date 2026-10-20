import QtQuick
import QtQuick.Effects

// A short notice: "info", "success" or "error". It leaves by itself after
// `timeout` ms; hovering or focusing it pauses the timer. An optional
// action. Screen readers announce it. Usually shown through ToastStack.
FocusScope {
    id: root

    property string kind: "info"
    property string title
    property string body
    // The action's label; no action when empty.
    property string actionText
    // 0 keeps it until dismissed (the gallery's still pictures).
    property int timeout: 4000
    // Shows the paused look without the mouse, for the gallery.
    property bool previewPaused: false
    readonly property bool paused: hover.hovered || root.activeFocus || previewPaused
    // 1 → 0 as the time runs out.
    property real remaining: 1
    readonly property color tone: kind === "success" ? Theme.success : kind === "error" ? Theme.danger : Theme.info

    signal dismissed
    signal actionTriggered

    implicitWidth: Theme.space48 * 8 - Theme.space4
    implicitHeight: content.implicitHeight + Theme.space24
    Accessible.role: Accessible.AlertMessage
    Accessible.name: title
    Accessible.description: body

    NumberAnimation on remaining {
        id: countdown

        from: 1
        to: 0
        duration: root.timeout
        running: root.timeout > 0
        paused: running && root.paused
        onFinished: root.dismissed()
    }

    HoverHandler {
        id: hover
    }

    RectangularShadow {
        anchors.fill: surface
        radius: Theme.radiusCard
        offset.y: Theme.elevationCardShadow1Y
        blur: Theme.elevationCardShadow1Blur
        color: Theme.elevationCardShadow1
    }

    RectangularShadow {
        anchors.fill: surface
        radius: Theme.radiusCard
        offset.y: Theme.elevationCardShadow2Y
        blur: Theme.elevationCardShadow2Blur
        color: Theme.elevationCardShadow2
    }

    Rectangle {
        id: surface

        anchors.fill: parent
        radius: Theme.radiusCard
        color: Theme.bgSurface2
        border.width: Theme.elevationCardBorder.a > 0 ? 1 : 0
        border.color: Theme.elevationCardBorder
        clip: true

        // The kind's stripe on the left.
        Rectangle {
            width: Theme.space2 + 1
            height: parent.height
            color: root.tone
        }
    }

    Row {
        id: content

        x: Theme.space12 + Theme.space2
        y: Theme.space12
        width: parent.width - x - Theme.space12
        spacing: Theme.space8 + Theme.space2

        Icon {
            id: kindIcon

            y: Theme.space2
            name: root.kind === "success" ? "check" : root.kind === "error" ? "triangle-alert" : "info"
            color: root.tone
        }

        Column {
            width: content.width - kindIcon.width - actions.width - 2 * content.spacing
            spacing: Theme.space2

            Text {
                width: parent.width
                text: root.title
                wrapMode: Text.Wrap
                font: Qt.font({
                    family: Theme.body.family,
                    pixelSize: Theme.body.pixelSize,
                    weight: Font.DemiBold
                })
                color: Theme.textPrimary
                Accessible.ignored: true
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: root.body
                wrapMode: Text.Wrap
                font: Theme.small
                color: Theme.textSecondary
                Accessible.ignored: true
            }

            // While paused: how much time is left.
            Item {
                visible: root.paused && root.timeout > 0 || root.previewPaused
                width: parent.width
                height: visible ? pausedNote.y + pausedNote.height : 0

                Rectangle {
                    y: Theme.space8
                    width: parent.width
                    height: Theme.space2
                    radius: Theme.space2
                    color: Theme.borderSubtle

                    Rectangle {
                        width: parent.width * root.remaining
                        height: parent.height
                        radius: parent.radius
                        color: root.tone
                    }
                }

                Text {
                    id: pausedNote

                    y: Theme.space8 + Theme.space2 + Theme.space2
                    text: qsTr("Paused while you're here")
                    font: Theme.caption
                    color: Theme.textTertiary
                    Accessible.ignored: true
                }
            }
        }

        Row {
            id: actions

            spacing: Theme.space4

            Button {
                visible: root.actionText !== ""
                size: "small"
                variant: "ghost"
                text: root.actionText
                onClicked: root.actionTriggered()
            }

            IconButton {
                size: "small"
                variant: "ghost"
                iconName: "x"
                accessibleName: qsTr("Dismiss")
                preview: root.previewPaused ? "hover" : ""
                onClicked: root.dismissed()
            }
        }
    }
}
