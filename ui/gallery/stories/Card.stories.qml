import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "Card"
    summary: "Surface 1, card corners, a soft shadow in light and a 1 px line in dark. Optional header and footer."

    Story {
        name: "compact"
        title: "Compact"

        Card {
            width: 560
            compact: true

            Text {
                text: "Docker is installed"
                font: Qt.font({
                    family: Theme.body.family,
                    pixelSize: Theme.body.pixelSize,
                    weight: Font.DemiBold
                })
                color: Theme.textPrimary
            }

            Text {
                text: "Started and set to run at login. 2 min ago"
                font: Theme.small
                color: Theme.textSecondary
            }
        }
    }

    Story {
        name: "expanded"
        title: "Expanded, header and footer"

        Card {
            width: 560
            title: "Install Docker"
            iconName: "package"
            footer: Row {
                spacing: Theme.space8

                Button {
                    variant: "ghost"
                    text: "Details"
                }

                Button {
                    variant: "primary"
                    text: "Do it"
                }
            }

            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "I'll install Docker from the Arch repositories, turn it on, and add you to the docker group."
                font: Theme.body
                color: Theme.textPrimary
            }

            Chip {
                mode: "sandbox"
            }
        }
    }
}
