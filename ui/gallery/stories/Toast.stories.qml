import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "Toast"
    summary: "Info, success and error. Leaves after 4 s; hovering or focusing pauses it. At most 3 on screen. Announced to screen readers."

    Story {
        name: "kinds"
        title: "Info · success · error"

        Column {
            spacing: Theme.space8

            Toast {
                width: 380
                timeout: 0
                kind: "info"
                title: "Settings reloaded"
                body: "You changed ~/.config/priya/settings.toml."
            }

            Toast {
                width: 380
                timeout: 0
                kind: "success"
                title: "Docker is installed"
                body: "Started and set to run at login."
                actionText: "Undo"
            }

            Toast {
                width: 380
                timeout: 0
                kind: "error"
                title: "Couldn't reach Anthropic"
                body: "Check your connection, then try again."
                actionText: "Try again"
            }
        }
    }

    Story {
        name: "paused"
        title: "Hovered: paused"

        Toast {
            width: 380
            timeout: 0
            previewPaused: true
            remaining: 0.62
            kind: "success"
            title: "Backup finished"
            body: "42 files, 1.3 GB."
        }
    }

    Story {
        name: "long-hindi"
        title: "Long text and Hindi"

        Column {
            spacing: Theme.space8

            Toast {
                width: 380
                timeout: 0
                kind: "info"
                title: "An update is ready for 214 packages, including the Linux kernel and Mesa"
                body: "I'll check it in a sandbox first and tell you if anything breaks."
            }

            Toast {
                width: 380
                timeout: 0
                kind: "success"
                title: "सेटिंग्स बदल गईं"
                body: "थीम अब हल्की है।"
            }
        }
    }
}
