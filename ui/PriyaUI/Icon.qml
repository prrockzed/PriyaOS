import QtQuick
import QtQuick.Effects

// A Lucide icon from icons/, tinted with a Theme colour (11 §2.5). It is
// decoration: the control around it carries the spoken name, and an icon
// that means something always comes with a word.
Item {
    id: root

    // The file name in icons/ without ".svg", e.g. "copy".
    property string name
    // 16, 20 or 24 at scale 1.0.
    property int size: 16
    property color color: Theme.textPrimary

    implicitWidth: size * Theme.scale
    implicitHeight: size * Theme.scale
    Accessible.ignored: true

    Image {
        id: glyph

        anchors.fill: parent
        source: root.name ? Qt.resolvedUrl("icons/" + root.name + ".svg") : ""
        sourceSize: Qt.size(width, height)
        smooth: true
        visible: false
    }

    // The SVGs draw in black (currentColor); brightness lifts that to white
    // so colorization gives exactly `color`.
    MultiEffect {
        anchors.fill: glyph
        source: glyph
        brightness: 1
        colorization: 1
        colorizationColor: root.color
    }
}
