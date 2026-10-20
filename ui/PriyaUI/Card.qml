pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Effects

// The surface other content sits on: surface 1, card corners, a soft
// shadow in light and a 1 px line in dark. Optional header (a title with an
// icon, or any component) and footer (usually buttons, right-aligned).
// Children go in the body.
Item {
    id: root

    property bool compact: false
    // A simple header: an icon and a bold title.
    property string title
    property string iconName
    // Or any header or footer.
    property Component header
    property Component footer
    default property alias content: body.data
    readonly property bool hasHeader: header !== null || title !== ""

    implicitWidth: Theme.space48 * 11 + Theme.space32
    implicitHeight: column.implicitHeight

    // Light: two soft shadow layers. Dark: their colours are transparent.
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
        color: Theme.bgSurface1
        border.width: Theme.elevationCardBorder.a > 0 ? 1 : 0
        border.color: Theme.elevationCardBorder
    }

    Column {
        id: column

        width: root.width

        Item {
            visible: root.hasHeader
            width: parent.width
            height: visible ? headerLoader.height + Theme.space24 : 0

            Loader {
                id: headerLoader

                x: Theme.space16
                width: parent.width - Theme.space32
                anchors.verticalCenter: parent.verticalCenter
                sourceComponent: root.header ?? titleRow
            }

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: Theme.borderSubtle
            }
        }

        Item {
            width: parent.width
            height: body.implicitHeight + 2 * body.y

            Column {
                id: body

                x: Theme.space16
                y: root.compact ? Theme.space12 : Theme.space16
                width: parent.width - Theme.space32
                spacing: Theme.space8
            }
        }

        Item {
            visible: root.footer !== null
            width: parent.width
            height: visible ? footerLoader.height + Theme.space16 : 0

            Rectangle {
                width: parent.width
                height: 1
                color: Theme.borderSubtle
            }

            Loader {
                id: footerLoader

                anchors.right: parent.right
                anchors.rightMargin: Theme.space12
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(implicitWidth, parent.width - Theme.space24)
                sourceComponent: root.footer
            }
        }
    }

    Component {
        id: titleRow

        Row {
            spacing: Theme.space8

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: root.iconName
                visible: root.iconName !== ""
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.title
                font: Qt.font({
                    family: Theme.body.family,
                    pixelSize: Theme.body.pixelSize,
                    weight: Font.DemiBold
                })
                color: Theme.textPrimary
                Accessible.role: Accessible.Heading
                Accessible.name: text
            }
        }
    }
}
