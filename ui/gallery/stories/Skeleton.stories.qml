import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "Skeleton"
    summary: "Soft shimmering lines while Priya is thinking, instead of a spinner. Still with reduced motion (and in screenshots)."

    Story {
        name: "thinking"
        title: "Thinking"

        Skeleton {
            width: 420
        }
    }
}
