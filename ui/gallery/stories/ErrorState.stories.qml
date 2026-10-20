import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "ErrorState"
    summary: "What happened (title) and what to do next (body). Try again, and Details with copyable technical text. Never blames the user."

    Story {
        name: "collapsed"
        title: "Collapsed"

        Card {
            width: 560

            ErrorState {
                width: parent.width
                title: "Priya can't reach the model"
                body: "Your internet connection seems to be off. Connect, then try again."
                details: "kind: unavailable\nprovider: anthropic\nerror: dns error: failed to lookup address information: Temporary failure in name resolution\nrequest: 01JABCDEFGHJKMNPQRSTVWXYZ0"
            }
        }
    }

    Story {
        name: "details"
        title: "Details open"

        Card {
            width: 560

            ErrorState {
                width: parent.width
                detailsOpen: true
                title: "Priya can't reach the model"
                body: "Your internet connection seems to be off. Connect, then try again."
                details: "kind: unavailable\nprovider: anthropic\nerror: dns error: failed to lookup address information: Temporary failure in name resolution\nrequest: 01JABCDEFGHJKMNPQRSTVWXYZ0"
            }
        }
    }

    Story {
        name: "hindi"
        title: "Hindi"

        Card {
            width: 560

            ErrorState {
                width: parent.width
                title: "प्रिया मॉडल तक नहीं पहुँच पा रही"
                body: "लगता है इंटरनेट बंद है। जुड़ने के बाद फिर से कोशिश करें।"
                details: "kind: unavailable"
            }
        }
    }

    Story {
        name: "commands"
        title: "With a command to copy"

        Card {
            width: 560

            ErrorState {
                width: parent.width
                iconName: "triangle-alert"
                showRetry: false
                title: "Priya's background service isn't running"
                body: "Start it in a terminal. The bar connects again by itself."
                details: "kind: unavailable\nsocket: /run/user/1000/priya/priyad.sock\nerror: connection refused"

                CodeBlock {
                    width: parent.width
                    language: "bash"
                    code: "systemctl --user start priyad"
                }

                Text {
                    width: parent.width
                    textFormat: Text.PlainText
                    text: "Trying again in 5 s…"
                    font: Theme.caption
                    color: Theme.textTertiary
                }
            }
        }
    }
}
