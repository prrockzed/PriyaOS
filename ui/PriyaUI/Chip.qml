import QtQuick
import QtQuick.Controls.Basic as T

// A small label with an optional icon. Mode chips always pair the mode's
// colour with an icon and a word, never colour alone: "look" (read-only),
// "sandbox", "direct", "direct_raw". Selectable chips toggle with a click,
// Space or Enter and show a check when selected.
T.AbstractButton {
    id: root

    // "" for a neutral chip, or a mode.
    property string mode
    property string iconName
    property bool selectable: false
    // Shows a state without the mouse or keyboard, for the gallery: "hover"
    // or "focus".
    property string preview
    property alias selected: root.checked

    readonly property var modes: ({
            "look": {
                "color": Theme.modeLook,
                "icon": "eye",
                "label": qsTr("Look")
            },
            "sandbox": {
                "color": Theme.modeSandbox,
                "icon": "box",
                "label": qsTr("Sandbox")
            },
            "direct": {
                "color": Theme.modeDirect,
                "icon": "zap",
                "label": qsTr("Direct")
            },
            "direct_raw": {
                "color": Theme.modeDirectRaw,
                "icon": "triangle-alert",
                "label": qsTr("Direct raw")
            }
        })
    readonly property var modeInfo: modes[mode] ?? null
    readonly property color tone: modeInfo ? modeInfo.color : Theme.textPrimary
    readonly property string shownIcon: selectable && checked ? "check" : modeInfo ? modeInfo.icon : iconName

    text: modeInfo ? modeInfo.label : ""
    checkable: selectable
    hoverEnabled: selectable
    focusPolicy: selectable ? Qt.StrongFocus : Qt.NoFocus
    activeFocusOnTab: selectable
    implicitHeight: Theme.space24 + Theme.space4
    implicitWidth: row.implicitWidth + leftPadding + rightPadding
    leftPadding: Theme.space8 + Theme.space2
    rightPadding: leftPadding
    font: Qt.font({
        family: Theme.small.family,
        pixelSize: Theme.small.pixelSize,
        weight: Font.Medium
    })

    Accessible.role: selectable ? Accessible.CheckBox : Accessible.StaticText
    Accessible.name: text
    Accessible.checkable: selectable
    Accessible.checked: checked

    Keys.onReturnPressed: event => root.toggleFromKey(event)
    Keys.onEnterPressed: event => root.toggleFromKey(event)

    function toggleFromKey(event) {
        event.accepted = selectable;
        if (selectable) {
            toggle();
            toggled();
            clicked();
        }
    }

    contentItem: Item {
        Row {
            id: row

            anchors.centerIn: parent
            spacing: Theme.space4 + Theme.space2

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: root.shownIcon
                visible: root.shownIcon !== ""
                color: root.tone
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.text
                font: root.font
                color: root.tone
                Accessible.ignored: true
            }
        }
    }

    background: Rectangle {
        radius: height / 2
        color: {
            if (root.modeInfo)
                return Qt.alpha(root.tone, 0.1);
            if (root.selectable && root.checked)
                return Theme.accentSubtle;
            if (root.selectable && (root.hovered || root.preview === "hover"))
                return Theme.bgSurface3;
            return Theme.bgSurface1;
        }
        border.width: 1
        border.color: root.modeInfo ? root.tone : root.selectable && root.checked ? Theme.accent : Theme.borderStrong
    }

    FocusRing {
        targetRadius: root.height / 2
        shown: root.preview === "focus" || (root.activeFocus && [Qt.TabFocusReason, Qt.BacktabFocusReason, Qt.ShortcutFocusReason].includes(root.focusReason))
    }
}
