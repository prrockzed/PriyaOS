import QtQuick

// A soft band of light sweeping across its parent: Priya is working
// (11 §2.4, never a spinner). With reduced motion the band stands still in
// the middle.
Item {
    id: root

    property color band
    property bool running: true

    clip: true
    Accessible.ignored: true

    Rectangle {
        id: sweep

        width: root.width
        height: root.height
        x: Theme.animationsEnabled && root.running ? -root.width : 0
        gradient: Gradient {
            orientation: Gradient.Horizontal

            GradientStop {
                position: 0.2
                color: "transparent"
            }

            GradientStop {
                position: 0.5
                color: root.band
            }

            GradientStop {
                position: 0.8
                color: "transparent"
            }
        }

        NumberAnimation on x {
            from: -root.width
            to: root.width
            duration: 1400
            loops: Animation.Infinite
            running: Theme.animationsEnabled && root.running && root.visible
        }
    }
}
