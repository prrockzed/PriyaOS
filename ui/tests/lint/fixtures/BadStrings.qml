// Fixture for ui_lint_rejects_untranslated_string.
import QtQuick
import QtQuick.Controls.Basic as T

Item {
    property string text: "Keep changes" // BAD
    property string placeholderText: "Ask anything" // BAD
    Accessible.name: "Close" // BAD
    Accessible.description: "Copies the answer" // BAD
    T.ToolTip.text: "Copy" // BAD
    Text { text: on ? qsTr("On") : "Off" } // BAD
    Text { text: "Search" } // BAD
    Text { text: qsTr("Fine"); property string kind: "plain-id" }
    Text { text: "" }
    Text { text: "→ 12" }
    TextField { label: "Provider name" } // BAD
    EmptyState { title: "No tasks"; body: qsTr("Fine") } // BAD
}
