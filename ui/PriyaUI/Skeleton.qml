pragma ComponentBehavior: Bound

import QtQuick

// "Priya is thinking": soft shimmering lines instead of a spinner. With
// reduced motion the lines stay still.
Item {
    id: root

    property string label: qsTr("Thinking…")
    // Line widths as fractions of the width.
    property var lines: [0.92, 0.78, 0.54]
    readonly property bool animating: Theme.animationsEnabled && visible

    implicitHeight: column.implicitHeight
    implicitWidth: Theme.space48 * 8 + Theme.space32 + Theme.space4
    Accessible.role: Accessible.StaticText
    Accessible.name: label

    Column {
        id: column

        width: root.width
        spacing: Theme.space8

        Row {
            spacing: Theme.space8

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "sparkles"
                color: Theme.textSecondary
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.label
                font: Theme.small
                color: Theme.textSecondary
                Accessible.ignored: true
            }
        }

        Repeater {
            model: root.lines

            delegate: Rectangle {
                required property real modelData

                width: column.width * modelData
                height: Theme.space12
                radius: Theme.radiusControl
                color: root.animating ? Theme.bgSurface2 : Theme.bgSurface3
                clip: true

                Rectangle {
                    id: sweep

                    visible: root.animating
                    width: parent.width
                    height: parent.height
                    gradient: Gradient {
                        orientation: Gradient.Horizontal

                        GradientStop {
                            position: 0
                            color: Theme.bgSurface2
                        }

                        GradientStop {
                            position: 0.5
                            color: Theme.bgSurface3
                        }

                        GradientStop {
                            position: 1
                            color: Theme.bgSurface2
                        }
                    }

                    NumberAnimation on x {
                        from: -sweep.width
                        to: sweep.width
                        duration: 1600
                        loops: Animation.Infinite
                        running: root.animating
                    }
                }
            }
        }
    }
}
