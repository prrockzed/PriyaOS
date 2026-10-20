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
}
