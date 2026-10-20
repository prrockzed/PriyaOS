import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "Icon"
    summary: "Lucide icons, stroke 1.75, sizes 16 / 20 / 24, tinted with a Theme colour."

    Story {
        name: "sizes"
        title: "Sizes"

        Row {
            spacing: Theme.space12

            Icon {
                name: "sparkles"
                size: 16
            }

            Icon {
                name: "sparkles"
                size: 20
            }

            Icon {
                name: "sparkles"
                size: 24
            }
        }
    }

    Story {
        name: "tints"
        title: "Tints"

        Row {
            spacing: Theme.space12

            Icon {
                name: "info"
                size: 20
                color: Theme.textPrimary
            }

            Icon {
                name: "info"
                size: 20
                color: Theme.textSecondary
            }

            Icon {
                name: "sparkles"
                size: 20
                color: Theme.accent
            }

            Icon {
                name: "check"
                size: 20
                color: Theme.success
            }

            Icon {
                name: "triangle-alert"
                size: 20
                color: Theme.danger
            }
        }
    }
}
