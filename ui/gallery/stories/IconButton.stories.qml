import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "IconButton"
    summary: "A Button with only an icon. It needs a spoken name, also shown as a tooltip after 600 ms."

    Story {
        name: "default"
        title: "Default"

        Row {
            spacing: Theme.space8

            IconButton {
                variant: "ghost"
                iconName: "copy"
                accessibleName: "Copy"
            }

            IconButton {
                iconName: "x"
                accessibleName: "Close"
            }

            IconButton {
                variant: "primary"
                iconName: "arrow-up"
                accessibleName: "Send"
            }
        }
    }

    Story {
        name: "tooltip"
        title: "Hover + tooltip"

        Item {
            width: tipped.width
            height: tipped.height + Theme.space32

            IconButton {
                id: tipped

                y: Theme.space32
                variant: "ghost"
                iconName: "copy"
                accessibleName: "Copy"
                preview: "hover"
            }
        }
    }

    Story {
        name: "states"
        title: "Pressed · focus · disabled · busy"

        Row {
            spacing: Theme.space8

            IconButton {
                variant: "ghost"
                iconName: "copy"
                accessibleName: "Copy"
                preview: "pressed"
            }

            IconButton {
                variant: "ghost"
                iconName: "copy"
                accessibleName: "Copy"
                preview: "focus"
            }

            IconButton {
                variant: "ghost"
                iconName: "copy"
                accessibleName: "Copy"
                enabled: false
            }

            IconButton {
                variant: "primary"
                iconName: "arrow-up"
                accessibleName: "Send"
                busy: true
            }
        }
    }
}
