import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "PromptInput"
    summary: "Grows from 1 to 6 lines, then scrolls. Enter sends, Shift+Enter adds a line, Esc closes, Up brings back the last prompt."

    Story {
        name: "empty"
        title: "Empty"

        PromptInput {
            width: 560
            showHints: true
        }
    }

    Story {
        name: "typing"
        title: "Typing"

        PromptInput {
            width: 560
            text: "What's using my disk?"
            preview: "focus"
        }
    }

    Story {
        name: "grown"
        title: "Grown, 3 lines"

        PromptInput {
            width: 560
            text: "Set up a Python environment for the project in ~/code/app.\nUse uv, not pip.\nThen run the tests and tell me what fails."
            preview: "focus"
        }
    }

    Story {
        name: "scrolls"
        title: "Past 6 lines: scrolls"

        PromptInput {
            width: 560
            text: "Here is the error from the build:\nerror[E0308]: mismatched types\n  --> src/main.rs:12:5\n   |\n12 |     run(config)\n   |     ^^^^^^^^^^^ expected `Result<(), Error>`, found `()`\nWhat does it mean and how do I fix it?"
            preview: "focus"
        }
    }

    Story {
        name: "hindi"
        title: "Hindi"

        PromptInput {
            width: 560
            text: "मेरी डिस्क पर सबसे ज़्यादा जगह क्या ले रहा है?"
            preview: "focus"
        }
    }

    Story {
        name: "sending"
        title: "Disabled (sending)"

        PromptInput {
            width: 560
            text: "What's using my disk?"
            busy: true
        }
    }
}
