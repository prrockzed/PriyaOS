import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "KeyHints"
    summary: "Which keys do what, each key drawn as a small key cap. Inline under an input, or in the Priya bar's footer with keys joined by +."

    Story {
        name: "inline"
        title: "Inline, under an input"

        KeyHints {
            hints: [
                {
                    "keys": ["Enter"],
                    "label": "send"
                },
                {
                    "keys": ["Shift", "Enter"],
                    "label": "new line"
                },
                {
                    "keys": ["Esc"],
                    "label": "close"
                }
            ]
        }
    }

    Story {
        name: "footer"
        title: "In the Priya bar's footer"

        KeyHints {
            variant: "footer"
            hints: [
                {
                    "keys": ["Enter"],
                    "label": "ask"
                },
                {
                    "keys": ["Shift", "Enter"],
                    "label": "new line"
                },
                {
                    "keys": ["Esc"],
                    "label": "close"
                }
            ]
        }
    }
}
