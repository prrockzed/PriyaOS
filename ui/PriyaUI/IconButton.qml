import QtQuick
import QtQuick.Controls.Basic as T

// A Button with only an icon. It must have a spoken name, which is also
// its tooltip, shown after 600 ms of hover or keyboard focus.
Button {
    id: root

    required property string accessibleName

    text: ""
    Accessible.name: accessibleName

    T.ToolTip {
        id: tip

        objectName: "tooltip"
        parent: root
        // Above the button, centred.
        x: (root.width - width) / 2
        y: -height - Theme.space4 - Theme.space2
        visible: root.preview === "" && (root.hovered || root.visualFocus) && root.enabled
        delay: 600
        text: root.accessibleName
        padding: 0
        leftPadding: Theme.space8
        rightPadding: Theme.space8
        topPadding: Theme.space4
        bottomPadding: Theme.space4

        contentItem: Text {
            textFormat: Text.PlainText
            text: tip.text
            font: Theme.caption
            color: Theme.bgBase
            Accessible.ignored: true
        }

        background: Rectangle {
            radius: Theme.radiusControl
            color: Theme.textPrimary
        }
    }

    // The gallery's "hover" picture: the same bubble drawn in place, since a
    // popup lives outside the item and a still picture of it can't include
    // it.
    Rectangle {
        visible: root.preview === "hover"
        x: (root.width - width) / 2
        y: -height - Theme.space4 - Theme.space2
        width: previewText.implicitWidth + 2 * Theme.space8
        height: previewText.implicitHeight + 2 * Theme.space4
        radius: Theme.radiusControl
        color: Theme.textPrimary

        Text {
            id: previewText

            textFormat: Text.PlainText
            anchors.centerIn: parent
            text: root.accessibleName
            font: Theme.caption
            color: Theme.bgBase
            Accessible.ignored: true
        }
    }
}
