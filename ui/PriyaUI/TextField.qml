import QtQuick
import QtQuick.Controls.Basic as T

// A one-line text field with a label above it. States: empty (placeholder),
// focused, filled, disabled and error, whose message shows under the field
// and is read out as the field's description. Long text is cut with …
// until the field is focused, then it scrolls.
FocusScope {
    id: root

    // The spoken and visible name, e.g. "Provider name".
    property string label
    property alias text: field.text
    property alias placeholderText: field.placeholderText
    // Empty when the value is fine; otherwise what to change.
    property string errorText
    // Shows the focused look without focus, for the gallery: "focus".
    property string preview
    readonly property alias editor: field
    readonly property bool showFocus: field.activeFocus || preview === "focus"
    readonly property bool hasError: errorText !== ""

    signal accepted

    implicitWidth: Theme.space48 * 7 + Theme.space24
    implicitHeight: column.implicitHeight

    Column {
        id: column

        width: root.width
        spacing: Theme.space4

        Text {
            textFormat: Text.PlainText
            text: root.label
            visible: root.label !== ""
            font: Qt.font({
                family: Theme.small.family,
                pixelSize: Theme.small.pixelSize,
                weight: Font.Medium
            })
            color: Theme.textPrimary
            Accessible.ignored: true
        }

        T.TextField {
            id: field

            width: parent.width
            height: Theme.space32 + Theme.space4
            focus: true
            activeFocusOnTab: true
            enabled: root.enabled
            opacity: enabled ? 1 : 0.45
            leftPadding: Theme.space12
            rightPadding: Theme.space12
            topPadding: 0
            bottomPadding: 0
            verticalAlignment: TextInput.AlignVCenter
            font: Theme.body
            // Hidden behind the elided copy until the field is edited.
            color: activeFocus ? Theme.textPrimary : "transparent"
            placeholderTextColor: Theme.textTertiary
            selectionColor: Theme.accentSubtle
            selectedTextColor: Theme.textPrimary
            Accessible.role: Accessible.EditableText
            Accessible.name: root.label !== "" ? root.label : placeholderText
            Accessible.description: root.errorText
            onAccepted: root.accepted()

            background: Rectangle {
                radius: Theme.radiusControl
                color: Theme.bgSurface1
                border.width: root.showFocus || root.hasError ? 2 : 1
                border.color: root.hasError ? Theme.danger : root.showFocus ? Theme.accent : Theme.borderStrong
            }

            Text {
                textFormat: Text.PlainText
                anchors.fill: parent
                anchors.leftMargin: field.leftPadding
                anchors.rightMargin: field.rightPadding
                verticalAlignment: Text.AlignVCenter
                visible: !field.activeFocus
                text: field.text
                font: field.font
                color: Theme.textPrimary
                elide: Text.ElideRight
                Accessible.ignored: true
            }

            // The focused look in a still picture: a caret after the text.
            Rectangle {
                visible: root.preview === "focus" && !field.activeFocus
                x: field.leftPadding + Math.min(previewText.advanceWidth, field.width - field.leftPadding - field.rightPadding)
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(1, Theme.scale * 1.5)
                height: field.font.pixelSize * 1.1
                color: Theme.accent
            }

            TextMetrics {
                id: previewText

                font: field.font
                text: field.text
            }
        }

        Row {
            visible: root.hasError
            spacing: Theme.space4 + Theme.space2
            width: parent.width

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "triangle-alert"
                color: Theme.danger
            }

            Text {
                textFormat: Text.PlainText
                width: parent.width - Theme.space24
                text: root.errorText
                font: Theme.small
                color: Theme.danger
                wrapMode: Text.Wrap
                Accessible.ignored: true
            }
        }
    }
}
