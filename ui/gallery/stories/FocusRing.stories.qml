import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "FocusRing"
    summary: "2 px accent, 2 px outside the control, only for keyboard focus, never after a click."

    Story {
        name: "keyboard"
        title: "Keyboard focus · clicked"

        Row {
            spacing: Theme.space16

            Button {
                text: "Keyboard focus"
                preview: "focus"
            }

            Button {
                text: "Clicked: no ring"
            }
        }
    }
}
