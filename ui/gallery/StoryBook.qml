import QtQuick
import PriyaUI

// A component's stories (ui/gallery/stories/<Component>.stories.qml): its
// Story items, in the order the gallery lists them.
Item {
    id: root

    // The component's name, e.g. "Button".
    required property string component
    // One line on what it is.
    property string summary
    // Hindi sample text instead of English, where a story has some.
    property bool hindi: false
    default property list<Story> stories

    function story(name) {
        for (let i = 0; i < stories.length; ++i)
            if (stories[i].name === name)
                return stories[i];
        return null;
    }
}
