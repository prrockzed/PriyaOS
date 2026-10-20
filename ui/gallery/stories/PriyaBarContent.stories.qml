import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "PriyaBarContent"
    summary: "What the Priya bar shows, without its window: the mark and the input, the result, and key hints. Every state from the approved mockup."

    Story {
        name: "idle"
        title: "Idle, just opened"

        PriyaBarContent {
            width: 720
        }
    }

    Story {
        name: "typing"
        title: "Typing"

        PriyaBarContent {
            width: 720
            Component.onCompleted: input.text = "What's using my disk?"
        }
    }
}
