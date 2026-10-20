import QtQuick
import QtQuick.Shapes

// The Priya mark (11 §2): a soft disc, an open ring and a dot, tinted by
// state. Static for now; the state animations come in 5.x (UX-17).
Item {
    id: mark

    // "ready" (accent), "quiet" (stopped, nothing set up yet), "warning"
    // (offline) or "error".
    property string mood: "ready"
    property int size: 28
    readonly property color tint: mood === "error" ? Theme.danger : mood === "warning" ? Theme.warning : mood === "quiet" ? Theme.textTertiary : Theme.accent

    implicitWidth: size * Theme.scale
    implicitHeight: size * Theme.scale
    // Named like the mockup's mark; the bar says its state in words.
    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Priya")

    Shape {
        id: shape

        // Drawn on a 26 × 26 grid, like the mockup's SVG, in the middle
        // of the item.
        readonly property real unit: Math.min(mark.width, mark.height) / 26

        width: 26 * unit
        height: 26 * unit
        anchors.centerIn: parent
        // The default renderer smooths edges only with multisampling, which
        // the bar's window doesn't have; the curve renderer always does.
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            strokeColor: "transparent"
            fillColor: Qt.alpha(mark.tint, 0.15)

            PathAngleArc {
                centerX: 13 * shape.unit
                centerY: 13 * shape.unit
                radiusX: 11 * shape.unit
                radiusY: 11 * shape.unit
                startAngle: 0
                sweepAngle: 360
            }
        }

        ShapePath {
            strokeColor: mark.tint
            strokeWidth: 2.2 * shape.unit
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap

            // Three quarters of a circle, open at the top left.
            PathAngleArc {
                centerX: 13 * shape.unit
                centerY: 13 * shape.unit
                radiusX: 7.5 * shape.unit
                radiusY: 7.5 * shape.unit
                startAngle: -90
                sweepAngle: 270
            }
        }

        ShapePath {
            strokeColor: "transparent"
            fillColor: mark.tint

            PathAngleArc {
                centerX: 13 * shape.unit
                centerY: 13 * shape.unit
                radiusX: 2.4 * shape.unit
                radiusY: 2.4 * shape.unit
                startAngle: 0
                sweepAngle: 360
            }
        }
    }
}
