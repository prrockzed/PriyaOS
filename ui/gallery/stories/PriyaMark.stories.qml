import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "PriyaMark"
    summary: "The Priya mark: a soft disc, an open ring and a dot, tinted by the bar's state. It sits left of the Priya bar's input."

    Story {
        name: "moods"
        title: "Ready, quiet, warning, error"

        Row {
            spacing: Theme.space16

            PriyaMark {
                mood: "ready"
            }

            PriyaMark {
                mood: "quiet"
            }

            PriyaMark {
                mood: "warning"
            }

            PriyaMark {
                mood: "error"
            }
        }
    }

    Story {
        name: "large"
        title: "Large"

        PriyaMark {
            size: 64
        }
    }
}
