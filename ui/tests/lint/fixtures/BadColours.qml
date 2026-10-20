// Fixture for ui_lint_rejects_color_literal: each line marked BAD breaks
// the tokens-only rule.
import QtQuick
import PriyaUI

Rectangle {
    color: "#FF0000" // BAD
    border.color: "white" // BAD
    property color a: Qt.rgba(1, 0, 0, 1) // BAD
    property string css: "x" + rgba(1, 2, 3) // BAD
    Text { font.pixelSize: 14 } // BAD
    Text { color: "transparent"; font: Theme.body; text: qsTr("Fine") }
    Text { font.pointSize: 10.5 } // BAD
}
