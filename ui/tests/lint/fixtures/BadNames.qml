// Fixture for ui_lint_rejects_missing_accessible_name.
import QtQuick
import QtQuick.Controls.Basic as T
Item {
    MouseArea { anchors.fill: parent } // BAD
    Item {
        T.Button {} // BAD
        Rectangle { activeFocusOnTab: true } // BAD
    }
    Item {
        Accessible.name: qsTr("Toolbar")
        IconButton { icon: "copy" } // BAD
        Button { variant: "primary" } // BAD
        TextField { placeholderText: qsTr("Name") } // BAD
        MouseArea { anchors.fill: parent }
        IconButton { accessibleName: qsTr("Close") }
        Button { text: qsTr("Undo") }
        TextField { label: qsTr("Provider name") }
        Chip { text: qsTr("Python") }
    }
}
