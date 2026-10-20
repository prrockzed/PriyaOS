import QtQuick

// The keyboard focus ring (11 §7): 2 px of accent, 2 px outside the
// control, shown only when focus came from the keyboard (Tab, Shift+Tab, a
// shortcut), never after a click. Put it inside the control it outlines.
Rectangle {
    id: ring

    // The item it outlines; normally its parent.
    property Item target: parent
    // The target's corner radius, so the ring follows its shape.
    property real targetRadius: Theme.radiusControl
    // Keyboard focus only. Controls, TextField and TextArea all report
    // focusReason; a plain Item doesn't.
    property bool shown: target !== null && target.activeFocus && [Qt.TabFocusReason, Qt.BacktabFocusReason, Qt.ShortcutFocusReason].includes(target.focusReason) // qmllint disable missing-property

    x: -2 * Theme.focusWidth
    y: -2 * Theme.focusWidth
    width: target ? target.width + 4 * Theme.focusWidth : 0
    height: target ? target.height + 4 * Theme.focusWidth : 0
    radius: targetRadius + 2 * Theme.focusWidth
    color: "transparent"
    border.width: Theme.focusWidth
    border.color: Theme.focus
    visible: shown
    Accessible.ignored: true
}
