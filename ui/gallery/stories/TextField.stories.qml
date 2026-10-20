import QtQuick
import PriyaUI
import ".."

StoryBook {
    id: book

    component: "TextField"
    summary: "One line with a label. Empty, focused, filled, disabled, error (read out too), long text and Hindi."

    Story {
        name: "empty"
        title: "Empty"

        TextField {
            width: 360
            label: "Provider name"
            placeholderText: "For example, ollama"
        }
    }

    Story {
        name: "focused"
        title: "Focused"

        TextField {
            width: 360
            label: "Provider name"
            text: "olla"
            preview: "focus"
        }
    }

    Story {
        name: "filled"
        title: "Filled"

        TextField {
            width: 360
            label: "Provider name"
            text: "ollama"
        }
    }

    Story {
        name: "disabled"
        title: "Disabled"

        TextField {
            width: 360
            label: "Provider name"
            text: "anthropic"
            enabled: false
        }
    }

    Story {
        name: "error"
        title: "Error"

        TextField {
            width: 360
            label: "Provider name"
            text: "My Ollama"
            errorText: "Use lowercase letters, digits, - and _."
        }
    }

    Story {
        name: "long"
        title: "Long text"

        TextField {
            width: 360
            label: "Address"
            text: "http://192.168.1.20:1234/v1/models/meta-llama/Llama-3.1-8B-Instruct"
        }
    }

    Story {
        name: "hindi"
        title: "Hindi"

        TextField {
            width: 360
            label: "नाम"
            text: "प्रियंशु का लैपटॉप"
        }
    }
}
