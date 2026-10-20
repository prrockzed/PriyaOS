pragma ComponentBehavior: Bound

import QtQuick

// Key hints, each key drawn as a small key cap. "inline" (under an input):
// hints apart by "·", a hint's keys side by side. "footer" (the Priya bar's
// footer, as in its mockup): hints apart by a gap, keys joined by "+".
// Screen readers get `text` from the control the hints belong to, so the
// row itself is skipped.
Row {
    id: root

    // [{keys: ["Shift", "Enter"], label: "new line"}, …], translated.
    property var hints: []
    property string variant: "inline"
    // The hints as one line of text, for an Accessible.description.
    readonly property string text: hints.map(hint => hint.keys.join("+") + " " + hint.label).join(" · ")

    spacing: variant === "footer" ? Theme.space12 : Theme.space4
    Accessible.ignored: true

    Repeater {
        model: root.hints

        delegate: Row {
            id: hint

            required property var modelData
            required property int index

            spacing: Theme.space4

            Text {
                textFormat: Text.PlainText
                visible: hint.index > 0 && root.variant !== "footer"
                text: "·"
                font: Theme.caption
                color: Theme.textTertiary
                Accessible.ignored: true
            }

            Repeater {
                model: hint.modelData.keys

                delegate: Row {
                    id: key

                    required property string modelData
                    required property int index

                    spacing: Theme.space2

                    Text {
                        textFormat: Text.PlainText
                        visible: key.index > 0 && root.variant === "footer"
                        anchors.verticalCenter: parent.verticalCenter
                        text: "+"
                        font: Theme.caption
                        color: Theme.textTertiary
                        Accessible.ignored: true
                    }

                    Rectangle {
                        id: cap

                        objectName: "cap"
                        width: capText.implicitWidth + Theme.space8
                        height: capText.implicitHeight + Theme.space2
                        radius: Theme.space4
                        color: "transparent"
                        border.width: 1
                        border.color: Theme.borderSubtle

                        Text {
                            id: capText

                            textFormat: Text.PlainText

                            anchors.centerIn: parent
                            text: key.modelData
                            font: Qt.font({
                                family: Theme.fontMono,
                                pixelSize: Theme.caption.pixelSize
                            })
                            color: Theme.textSecondary
                            Accessible.ignored: true
                        }
                    }
                }
            }

            Text {
                textFormat: Text.PlainText
                text: hint.modelData.label
                font: Theme.caption
                color: Theme.textTertiary
                Accessible.ignored: true
            }
        }
    }
}
