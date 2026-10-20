import QtQuick
import PriyaUI
import ".."

StoryBook {
    id: book

    component: "Button"
    summary: "Primary, secondary, ghost and danger. Enter, Return and Space activate it; busy keeps the label and shimmers."

    Story {
        name: "default"
        title: "Default"

        Row {
            spacing: Theme.space8

            Button {
                variant: "primary"
                text: book.hindi ? "कायम रखें" : "Keep changes"
            }

            Button {
                variant: "secondary"
                text: book.hindi ? "वापस लें" : "Undo"
            }

            Button {
                variant: "ghost"
                text: book.hindi ? "विवरण" : "Details"
            }

            Button {
                variant: "danger"
                text: book.hindi ? "स्नैपशॉट मिटाएँ" : "Delete snapshot"
            }
        }
    }

    Story {
        name: "hover"
        title: "Hover"

        Row {
            spacing: Theme.space8

            Button {
                variant: "primary"
                text: book.hindi ? "कायम रखें" : "Keep changes"
                preview: "hover"
            }

            Button {
                variant: "secondary"
                text: book.hindi ? "वापस लें" : "Undo"
                preview: "hover"
            }

            Button {
                variant: "ghost"
                text: book.hindi ? "विवरण" : "Details"
                preview: "hover"
            }

            Button {
                variant: "danger"
                text: book.hindi ? "स्नैपशॉट मिटाएँ" : "Delete snapshot"
                preview: "hover"
            }
        }
    }

    Story {
        name: "pressed"
        title: "Pressed"

        Row {
            spacing: Theme.space8

            Button {
                variant: "primary"
                text: book.hindi ? "कायम रखें" : "Keep changes"
                preview: "pressed"
            }

            Button {
                variant: "secondary"
                text: book.hindi ? "वापस लें" : "Undo"
                preview: "pressed"
            }

            Button {
                variant: "ghost"
                text: book.hindi ? "विवरण" : "Details"
                preview: "pressed"
            }

            Button {
                variant: "danger"
                text: book.hindi ? "स्नैपशॉट मिटाएँ" : "Delete snapshot"
                preview: "pressed"
            }
        }
    }

    Story {
        name: "focus"
        title: "Keyboard focus"

        Row {
            spacing: Theme.space8

            Button {
                variant: "primary"
                text: book.hindi ? "कायम रखें" : "Keep changes"
                preview: "focus"
            }

            Button {
                variant: "secondary"
                text: book.hindi ? "वापस लें" : "Undo"
                preview: "focus"
            }

            Button {
                variant: "ghost"
                text: book.hindi ? "विवरण" : "Details"
                preview: "focus"
            }

            Button {
                variant: "danger"
                text: book.hindi ? "स्नैपशॉट मिटाएँ" : "Delete snapshot"
                preview: "focus"
            }
        }
    }

    Story {
        name: "disabled"
        title: "Disabled"

        Row {
            spacing: Theme.space8

            Button {
                variant: "primary"
                text: book.hindi ? "कायम रखें" : "Keep changes"
                enabled: false
            }

            Button {
                variant: "secondary"
                text: book.hindi ? "वापस लें" : "Undo"
                enabled: false
            }

            Button {
                variant: "ghost"
                text: book.hindi ? "विवरण" : "Details"
                enabled: false
            }

            Button {
                variant: "danger"
                text: book.hindi ? "स्नैपशॉट मिटाएँ" : "Delete snapshot"
                enabled: false
            }
        }
    }

    Story {
        name: "busy"
        title: "Busy"

        Row {
            spacing: Theme.space8

            Button {
                variant: "primary"
                text: book.hindi ? "कायम रखें" : "Keep changes"
                busy: true
            }

            Button {
                variant: "secondary"
                text: book.hindi ? "वापस लें" : "Undo"
                busy: true
            }

            Button {
                variant: "ghost"
                text: book.hindi ? "विवरण" : "Details"
                busy: true
            }

            Button {
                variant: "danger"
                text: book.hindi ? "स्नैपशॉट मिटाएँ" : "Delete snapshot"
                busy: true
            }
        }
    }

    Story {
        name: "icon"
        title: "With icon"

        Row {
            spacing: Theme.space8

            Button {
                variant: "primary"
                iconName: "check"
                text: book.hindi ? "एक बार अनुमति दें" : "Allow once"
            }

            Button {
                iconName: "undo-2"
                text: book.hindi ? "वापस लें" : "Undo"
            }
        }
    }

    Story {
        name: "small"
        title: "Small"

        Row {
            spacing: Theme.space8

            Button {
                size: "small"
                variant: "ghost"
                iconName: "copy"
                text: "Copy answer"
            }

            Button {
                size: "small"
                iconName: "refresh-cw"
                text: "Try again"
            }
        }
    }
}
