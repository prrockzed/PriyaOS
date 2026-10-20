import QtQuick
import PriyaUI

// One named state of a component in the gallery: fixed props, fixed text,
// no timers, so a screenshot of it is always the same.
Item {
    id: root

    // The state's id in shot names: "default", "hover", "hindi"…
    required property string name
    // What the gallery shows next to it; the name when empty.
    property string title
    // The scene: its children.
    default property alias content: holder.data
    // Space around the scene (screenshots include it).
    property real padding: Theme.space16

    implicitWidth: holder.childrenRect.width + 2 * padding
    implicitHeight: holder.childrenRect.height + 2 * padding

    Item {
        id: holder

        x: root.padding
        y: root.padding
        width: root.width - 2 * root.padding
        height: childrenRect.height
    }
}
