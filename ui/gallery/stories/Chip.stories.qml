import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "Chip"
    summary: "A label with an optional icon. Mode chips pair a colour with an icon and a word, never colour alone. Static or selectable."

    Story {
        name: "neutral"
        title: "Neutral"

        Row {
            spacing: Theme.space8

            Chip {
                text: "Python"
            }

            Chip {
                iconName: "folder"
                text: "~/code/app"
            }
        }
    }

    Story {
        name: "modes"
        title: "Modes"

        Row {
            spacing: Theme.space8

            Chip {
                mode: "look"
            }

            Chip {
                mode: "sandbox"
            }

            Chip {
                mode: "direct"
            }

            Chip {
                mode: "direct_raw"
            }
        }
    }

    Story {
        name: "selectable"
        title: "Selectable"

        Row {
            spacing: Theme.space8

            Chip {
                selectable: true
                selected: true
                text: "Cheapest"
            }

            Chip {
                selectable: true
                text: "Best"
            }

            Chip {
                selectable: true
                text: "Balanced"
                preview: "hover"
            }

            Chip {
                selectable: true
                text: "Local only"
                preview: "focus"
            }
        }
    }
}
