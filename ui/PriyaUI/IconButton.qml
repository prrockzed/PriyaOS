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
        visible: root.preview === "hover" || ((root.hovered || root.visualFocus) && root.enabled)
        delay: root.preview === "hover" ? 0 : 600
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
}
