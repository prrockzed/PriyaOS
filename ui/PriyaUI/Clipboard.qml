pragma Singleton

import QtQuick

// Copies text to the system clipboard. Qt Quick has no clipboard API, so
// it goes through a hidden editor's copy().
Item {
    function copy(text) {
        editor.text = text;
        editor.selectAll();
        editor.copy();
        editor.text = "";
    }

    TextEdit {
        id: editor

        visible: false
        Accessible.ignored: true
    }
}
