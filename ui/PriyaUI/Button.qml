import QtQuick
import QtQuick.Controls.Basic as T

// A button (11 §6): a short, verb-first label, optionally after an icon.
// Variants: "primary", "secondary", "ghost", "danger". Enter, Return and
// Space activate it. While `busy` the label stays, a soft shimmer runs
// across (still with reduced motion) and presses are ignored.
T.Button {
    id: root

    property string variant: "secondary"
    property bool busy: false
    // A Lucide icon name (icons/) shown before the label.
    property string iconName
    // "regular" (36 px tall) or "small" (28 px, inside cards and toasts).
    property string size: "regular"
    // Shows a state without the mouse or keyboard, for the gallery's fixed
    // stories: "hover", "pressed" or "focus".
    property string preview

    readonly property bool small: size === "small"
    readonly property bool iconOnly: text === "" && iconName !== ""
    readonly property bool filled: variant === "primary" || variant === "danger"
    readonly property bool showHover: (hovered || preview === "hover") && enabled && !busy
    readonly property bool showDown: (down || preview === "pressed") && enabled && !busy
    readonly property color foreground: filled ? Theme.textOnAccent : Theme.textPrimary

    implicitHeight: small ? Theme.space24 + Theme.space4 : Theme.space32 + Theme.space4
    implicitWidth: iconOnly ? implicitHeight : contentItem.implicitWidth + leftPadding + rightPadding
    leftPadding: iconOnly ? 0 : small ? Theme.space8 + Theme.space2 : Theme.space16
    rightPadding: leftPadding
    topPadding: 0
    bottomPadding: 0
    spacing: small ? Theme.space4 + Theme.space2 : Theme.space8
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus
    opacity: enabled ? 1 : 0.45
    font: Qt.font({
        family: (small ? Theme.small : Theme.body).family,
        pixelSize: (small ? Theme.small : Theme.body).pixelSize,
        weight: Font.Medium
    })

    Accessible.role: Accessible.Button
    Accessible.name: text
    Accessible.description: busy ? qsTr("Working…") : ""

    Keys.onReturnPressed: event => root.activate(event)
    Keys.onEnterPressed: event => root.activate(event)
    Keys.onSpacePressed: event => {
        // Swallowed while busy; otherwise the control handles Space.
        event.accepted = root.busy;
    }

    function activate(event) {
        event.accepted = true;
        // Holding Return doesn't repeat; animateClick shows the press.
        if (enabled && !busy && !event.isAutoRepeat)
            animateClick();
    }

    contentItem: Item {
        implicitWidth: row.implicitWidth
        implicitHeight: row.implicitHeight

        Row {
            id: row

            anchors.centerIn: parent
            spacing: root.spacing

            Icon {
                id: leading

                anchors.verticalCenter: parent.verticalCenter
                name: root.iconName
                visible: root.iconName !== ""
                color: root.foreground
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                // Cut with … when the button is narrower than its label.
                width: Math.min(implicitWidth, root.availableWidth - (leading.visible ? leading.width + row.spacing : 0))
                text: root.text
                visible: root.text !== ""
                font: root.font
                color: root.foreground
                elide: Text.ElideRight
                Accessible.ignored: true
            }
        }
    }

    background: Rectangle {
        radius: Theme.radiusControl
        color: {
            if (root.variant === "primary")
                return root.showHover ? root.shade(Theme.accent) : Theme.accent;
            if (root.variant === "danger")
                return root.showHover ? root.shade(Theme.danger) : Theme.danger;
            if (root.showHover || root.showDown)
                return Theme.bgSurface3;
            return root.variant === "ghost" ? "transparent" : Theme.bgSurface1;
        }
        border.width: root.variant === "secondary" ? 1 : 0
        border.color: Theme.borderStrong
        clip: true

        Behavior on color {
            ColorAnimation {
                duration: Theme.motionMicro
            }
        }

        // Pressed: a tint of the text colour over the fill.
        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: Qt.alpha(Theme.textPrimary, 0.08)
            visible: root.showDown
        }

        Shimmer {
            anchors.fill: parent
            visible: root.busy
            running: root.busy
            band: root.filled ? root.light() ? Qt.alpha(Theme.bgSurface1, 0.28) : Qt.alpha(Theme.textPrimary, 0.28) : Qt.alpha(Theme.textPrimary, root.light() ? 0.07 : 0.1)
        }
    }

    // A hover shade: darker in light mode, lighter in dark.
    function shade(colour) {
        return light() ? Qt.darker(colour, 1.08) : Qt.lighter(colour, 1.08);
    }

    function light() {
        return Theme.mode === "light";
    }

    // Swallows clicks while busy.
    MouseArea {
        anchors.fill: parent
        enabled: root.busy
        cursorShape: Qt.BusyCursor
        Accessible.ignored: true
    }

    FocusRing {
        targetRadius: Theme.radiusControl
        shown: root.preview === "focus" || (root.activeFocus && [Qt.TabFocusReason, Qt.BacktabFocusReason, Qt.ShortcutFocusReason].includes(root.focusReason))
    }
}
