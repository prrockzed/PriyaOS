import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "EmptyState"
    summary: "An icon, a title, a line of body text and an optional action, for screens with nothing in them yet."

    Story {
        name: "action"
        title: "With action"

        Card {
            width: 560

            EmptyState {
                width: parent.width
                iconName: "inbox"
                title: "No tasks yet"
                body: "Ask Priya to do something and it shows up here."
                actionText: "Open the Priya bar"
            }
        }
    }

    Story {
        name: "plain"
        title: "Without action"

        Card {
            width: 560

            EmptyState {
                width: parent.width
                iconName: "history"
                title: "Nothing to undo"
                body: "Changes Priya makes will appear here."
            }
        }
    }
}
