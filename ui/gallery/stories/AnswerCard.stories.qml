import QtQuick
import PriyaUI
import ".."

StoryBook {
    component: "AnswerCard"
    summary: "An answer as it streams: finished blocks never move, only the last one grows. Code blocks have a language label, Copy and sideways scroll."

    Story {
        name: "streaming"
        title: "Streaming"

        AnswerCard {
            width: 560
            model: "claude-sonnet-5-5"
            streaming: true
            text: "Your disk is mostly used by three things:\n\n- **Docker images**: 18.4 GB\n- **~/Videos**: 12.1 GB\n- The package cache: 6.2"
        }
    }

    Story {
        name: "code"
        title: "Done, with code"

        AnswerCard {
            width: 560
            model: "claude-sonnet-5-5"
            text: "To free the package cache, keep the last two versions of each package:\n\n```bash\nsudo paccache -rk2\n```\n\nThat frees about 5 GB. See the [Arch wiki on pacman](https://wiki.archlinux.org/title/Pacman) for more."
        }
    }

    Story {
        name: "error"
        title: "Error mid-stream"

        AnswerCard {
            width: 560
            model: "claude-sonnet-5-5"
            text: "Your disk is mostly used by three things:\n\n- **Docker images**: 18.4 GB"
            errorText: "The answer stopped: the connection to Anthropic dropped."
        }
    }

    Story {
        name: "long"
        title: "Long: scrolls inside"

        AnswerCard {
            width: 560
            model: "claude-sonnet-5-5"
            maxBodyHeight: 220
            text: "### Why your laptop gets hot\n\nThe fan runs fast when something keeps the processor busy. Right now that's mostly the Docker build you started at 21:40, which compiles a lot of code in parallel.\n\n### What you can do\n\n- Let the build finish; it should take about 6 min.\n- Limit Docker to 4 of your 8 cores in Settings → Developer.\n- Plug in: on battery, the laptop slows itself down to stay cool, which makes builds take longer.\n\nIf it still runs hot when nothing is running, ask me to look at the fan and temperature readings and I'll check for a stuck process.\n\nI can also show you what used the processor over the last hour."
        }
    }

    Story {
        name: "hindi"
        title: "Hindi"

        AnswerCard {
            width: 560
            model: "claude-sonnet-5-5"
            text: "आपकी डिस्क पर सबसे ज़्यादा जगह ये ले रहे हैं:\n\n- **Docker इमेज**: 18.4 GB\n- **~/Videos**: 12.1 GB\n\nपुराने पैकेज हटाने के लिए मुझसे कहें, मैं 5 GB तक खाली कर सकती हूँ।"
        }
    }
}
