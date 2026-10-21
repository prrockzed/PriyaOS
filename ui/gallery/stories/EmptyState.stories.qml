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

    Story {
        name: "commands"
        title: "With commands to copy"

        Card {
            width: 560

            EmptyState {
                width: parent.width
                iconName: "sparkles"
                title: "Connect a model"
                body: "Priya needs a model to answer. Run one of these in a terminal, then ask again."

                Text {
                    width: parent.width
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    text: "A free cloud model (Gemini). The first line asks for your key."
                    font: Theme.caption
                    color: Theme.textTertiary
                }

                CodeBlock {
                    width: parent.width
                    language: "bash"
                    code: "priya ai key set gemini\npriya settings set ai.strategy fixed:gemini/gemini-3.8-flash"
                }

                Text {
                    width: parent.width
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    text: "Or a model on this laptop (Ollama)."
                    font: Theme.caption
                    color: Theme.textTertiary
                }

                CodeBlock {
                    width: parent.width
                    language: "bash"
                    code: "priya settings set ai.strategy fixed:local/qwen2.5:0.5b"
                }
            }
        }
    }
}
