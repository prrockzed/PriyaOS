// Fixture for ui_lint_rejects_color_literal: each line marked BAD breaks
// the tokens-only rule.
import QtQuick
import PriyaUI

Rectangle {
    color: "#FF0000" // BAD
    border.color: "white" // BAD
    property color a: Qt.rgba(1, 0, 0, 1) // BAD
    property string css: "x" + rgba(1, 2, 3) // BAD
    Text { textFormat: Text.PlainText; font.pixelSize: 14 } // BAD
    Text { textFormat: Text.PlainText; color: "transparent"; font: Theme.body; text: qsTr("Fine") }
    Text { textFormat: Text.PlainText; font.pointSize: 10.5 } // BAD
    property color accent: "red" // BAD
    Rectangle { color: on ? "red" : Theme.accent } // BAD
    Text { textFormat: Text.PlainText; font.pixelSize: on ? 20 : Theme.body.pixelSize } // BAD
    Text { textFormat: Text.PlainText; font.pixelSize: Theme.body.pixelSize * 2; color: on ? Theme.accent : "transparent" }
}
