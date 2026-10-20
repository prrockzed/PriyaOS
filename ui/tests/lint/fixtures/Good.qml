// Fixture for ui_lint_accepts_good_code: follows every rule.
import QtQuick
import QtQuick.Controls.Basic as T
import PriyaUI

T.Button {
    id: root
    property string label: qsTr("Keep changes")
    text: label
    font: Theme.body
    Accessible.role: Accessible.Button
    Accessible.name: text
    T.ToolTip.text: qsTr("Keeps the changes")
    background: Rectangle {
        color: root.down ? Qt.alpha(Theme.textPrimary, 0.08) : "transparent"
        border.color: Theme.borderStrong
        radius: Theme.radiusControl
    }
    contentItem: Text {
        text: root.text
        color: Theme.textOnAccent
        font.pixelSize: Theme.body.pixelSize
        /* A comment with "#FF0000" and { braces */
    }
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: {
            const o = { text: "not a binding" }
            console.log(o.text)
        }
    }
    Accessible {
        // Grouped form, for completeness.
    }
    Text { text: root.state === "busy" ? qsTr("Working") : qsTr("Done") }
    Text { text: Qt.formatDateTime(new Date(), "hh:mm") }
    Text { text: qsTr("Open", "verb") }
    Text { text: model["title"] }
    Text { text: root.text === "Couldn't reach" ? qsTr("Yes") : qsTr("No") }
    MouseArea {
        hoverEnabled: true
        Accessible.ignored: true
    }
    function kindOf(k) {
        if (k === Kind.TextField) {
            return /^```/.test(k) ? "fence" : "text";
        }
        return "Ünïcödé …";
    }
}
