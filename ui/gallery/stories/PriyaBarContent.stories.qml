import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "PriyaBarContent"
    summary: "What the Priya bar shows, without its window: the mark and the input, the result, and key hints. Every state from the approved mockup."

    Story {
        name: "idle"
        title: "Idle, just opened"

        PriyaBarContent {
            width: 720
        }
    }

    Story {
        name: "typing"
        title: "Typing"

        PriyaBarContent {
            width: 720
            Component.onCompleted: input.text = "What's using my disk?"
        }
    }

    Story {
        name: "thinking"
        title: "Thinking"

        PriyaBarContent {
            width: 720
            phase: "thinking"
            question: "What's using my disk?"
        }
    }

    Story {
        name: "streaming"
        title: "Streaming"

        PriyaBarContent {
            width: 720
            phase: "streaming"
            question: "What's using my disk?"
            model: "gemini-3.8-flash"
            answer: "Your disk is mostly used by three things:\n\n- **Docker images**: 18.4 GB\n- **~/Videos**: 12.1 GB\n- The package cache: 6.2"
        }
    }

    Story {
        name: "done"
        title: "Done"

        PriyaBarContent {
            width: 720
            phase: "done"
            question: "What's using my disk?"
            model: "gemini-3.8-flash"
            answer: "Your disk is mostly used by three things:\n\n- **Docker images**: 18.4 GB\n- **~/Videos**: 12.1 GB\n- The package cache: 6.2 GB\n\nTo free the package cache, keep the last two versions of each package:\n\n```bash\nsudo paccache -rk2\n```"
        }
    }

    Story {
        name: "local"
        title: "Done, by the model on this laptop"

        PriyaBarContent {
            width: 720
            phase: "done"
            question: "What is btrfs?"
            model: "qwen2.5:0.5b"
            local: true
            answer: "**btrfs** is a Linux file system that can take instant snapshots: a saved copy of your files at one moment, which takes almost no space until files change. PriyaOS uses snapshots to undo changes."
        }
    }

    Story {
        name: "stopped"
        title: "Stopped"

        PriyaBarContent {
            width: 720
            phase: "stopped"
            question: "What's using my disk?"
            model: "gemini-3.8-flash"
            answer: "Your disk is mostly used by three things:\n\n- **Docker images**: 18.4 GB\n- **~/Videos**: 12.1 GB"
        }
    }

    Story {
        name: "long"
        title: "Long answer, scrolls"

        PriyaBarContent {
            width: 720
            phase: "done"
            maxHeight: 560
            question: "Why does my laptop get hot when I build things?"
            model: "gemini-3.8-flash"
            answer: "### Why it gets hot\n\nThe fan runs fast when something keeps the processor busy. A build compiles many files at once, using every core, so the processor runs at full power for minutes at a time.\n\n### What you can do\n\n- Plug in: on battery, the laptop slows itself down to stay cool, which makes builds take longer and stay hot longer.\n- Build with fewer cores at once, for example `cargo build -j 8`, so it runs a bit slower but cooler.\n- Keep the vents clear: a soft surface such as a bed or a blanket blocks the air.\n\n### When to worry\n\nHeat during a build is normal. If it stays hot when nothing is running, something may be stuck in the background. Ask me which programs are using the processor and I'll look.\n\nYou can also check the temperature yourself with the sensors command. Readings under 90 °C while building are fine for this kind of laptop.\n\nIf the fan is loud even when the laptop is cool, a firmware update from the maker sometimes changes how the fan behaves."
        }
    }

    Story {
        name: "hindi"
        title: "Hindi"

        PriyaBarContent {
            width: 720
            phase: "done"
            question: "मेरी डिस्क पर सबसे ज़्यादा जगह क्या ले रहा है?"
            model: "gemini-3.8-flash"
            answer: "आपकी डिस्क पर सबसे ज़्यादा जगह ये ले रहे हैं:\n\n- **Docker इमेज**: 18.4 GB\n- **~/Videos**: 12.1 GB\n- पैकेज कैश: 6.2 GB\n\nपुराने पैकेज हटाने के लिए यह चलाएँ:\n\n```bash\nsudo paccache -rk2\n```"
        }
    }

    Story {
        name: "busy"
        title: "The model is busy"

        PriyaBarContent {
            width: 720
            phase: "failed"
            providerLabel: "Gemini"
            error: ({
                    "kind": "provider_error",
                    "retryable": true,
                    "user_message": "Gemini is busy right now.",
                    "detail": "HTTP 503 from generativelanguage.googleapis.com"
                })
            Component.onCompleted: input.text = "What's using my disk?"
        }
    }

    Story {
        name: "offline"
        title: "Offline"

        PriyaBarContent {
            width: 720
            phase: "failed"
            providerLabel: "Gemini"
            error: ({
                    "kind": "unavailable",
                    "retryable": true,
                    "user_message": "Priya can't reach Gemini.",
                    "detail": "dns error: failed to lookup address information"
                })
            Component.onCompleted: input.text = "What's using my disk?"
        }
    }

    Story {
        name: "nomodel"
        title: "No model yet"

        PriyaBarContent {
            width: 720
            phase: "noModel"
            Component.onCompleted: input.text = "What's using my disk?"
        }
    }

    Story {
        name: "unavailable"
        title: "Background service off"

        PriyaBarContent {
            width: 720
            phase: "unavailable"
            retrySeconds: 5
        }
    }
}
