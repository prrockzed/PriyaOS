// AnswerCard, MarkdownBlocks.js and CodeBlock (0.11). Run with
// `cargo xtask test ui`.
import QtQuick
import QtTest
import PriyaUI
import "../../PriyaUI/MarkdownBlocks.js" as MarkdownBlocks
import ".."

Item {
    id: scene

    width: 640
    height: 800

    AnswerCard {
        id: answer

        x: 20
        y: 20
        width: 560
        model: "claude-sonnet-5-5"
    }

    CodeBlock {
        id: code

        x: 20
        y: 600
        width: 560
        language: "bash"
        code: "sudo paccache -rk2"
    }

    SignalSpy {
        id: links

        target: answer
        signalName: "linkActivated"
    }

    TextEdit {
        id: pasteTarget

        visible: false
    }

    A11yAudit {
        id: audit
    }

    TestCase {
        name: "AnswerCard"
        when: windowShown

        function init() {
            Theme.reducedMotion = true;
            answer.text = "";
            answer.streaming = false;
            answer.errorText = "";
        }

        function cleanup() {
            Theme.reducedMotion = false;
        }

        function kinds(text) {
            return MarkdownBlocks.split(text).map(b => b.kind);
        }

        function test_markdownblocks_split_cases_data() {
            return [
                {
                    "tag": "unclosed fence while streaming",
                    "text": "Run this:\n\n```bash\nsudo pacman -Syu\n",
                    "kinds": ["paragraph", "code"],
                    "last": {
                        "lang": "bash",
                        "source": "sudo pacman -Syu\n",
                        "open": true
                    }
                },
                {
                    "tag": "closed fence",
                    "text": "```\na\n\nb\n```\nafter",
                    "kinds": ["code", "paragraph"],
                    "first": {
                        "source": "a\n\nb",
                        "open": false
                    }
                },
                {
                    "tag": "nested lists",
                    "text": "- one\n  - one a\n  - one b\n- two\n\nDone.",
                    "kinds": ["list", "paragraph"],
                    "first": {
                        "source": "- one\n  - one a\n  - one b\n- two"
                    }
                },
                {
                    "tag": "Devanagari",
                    "text": "आपकी डिस्क पर सबसे ज़्यादा जगह ये ले रहे हैं:\n\n- **Docker इमेज**: 18.4 GB",
                    "kinds": ["paragraph", "list"],
                    "first": {
                        "source": "आपकी डिस्क पर सबसे ज़्यादा जगह ये ले रहे हैं:"
                    }
                },
                {
                    "tag": "link in a heading",
                    "text": "### See [the wiki](https://wiki.archlinux.org)\nText under it.",
                    "kinds": ["heading", "paragraph"],
                    "first": {
                        "source": "See [the wiki](https://wiki.archlinux.org)",
                        "level": 3
                    }
                },
                {
                    "tag": "quote then paragraph",
                    "text": "> quoted\n> more\n\nplain",
                    "kinds": ["quote", "paragraph"]
                },
                {
                    "tag": "fence with an info string",
                    "text": "```js title=x\nconst a = 1;\n```\nafter",
                    "kinds": ["code", "paragraph"],
                    "first": {
                        "lang": "js",
                        "source": "const a = 1;",
                        "open": false
                    }
                },
                {
                    "tag": "a hash inside a heading",
                    "text": "# C#\n\n## Title ##",
                    "kinds": ["heading", "heading"],
                    "first": {
                        "source": "C#",
                        "level": 1
                    },
                    "last": {
                        "source": "Title",
                        "level": 2
                    }
                },
                {
                    "tag": "Windows line ends",
                    "text": "a\r\nb\r\n\r\n```\r\nx\r\n```",
                    "kinds": ["paragraph", "code"],
                    "last": {
                        "source": "x"
                    }
                },
                {
                    "tag": "paragraph then list without a blank line",
                    "text": "Three things:\n- a\n- b",
                    "kinds": ["paragraph", "list"]
                }
            ];
        }

        // covers: DS-03
        // suite: ui
        // acceptance: 0.11
        function test_markdownblocks_split_cases(row) {
            const blocks = MarkdownBlocks.split(row.text);
            compare(blocks.map(b => b.kind), row.kinds);
            for (const [which, expected] of [["first", row.first], ["last", row.last]]) {
                if (!expected)
                    continue;
                const block = which === "first" ? blocks[0] : blocks[blocks.length - 1];
                for (const key in expected)
                    compare(block[key], expected[key], which + "." + key);
            }
        }

        // covers: DS-03
        // suite: ui
        // acceptance: 0.11
        function test_answercard_streaming_does_not_reflow_finished_blocks() {
            answer.streaming = true;
            const full = "Your disk is mostly used by three things:\n\n- **Docker images**: 18.4 GB\n- ~/Videos: 12.1 GB\n\nTo free space:\n\n```bash\nsudo paccache -rk2\n```\n\nThat frees about 5 GB.";
            let firstItem = null;
            let firstGeometry = null;
            let lastHeight = 0;
            // Stream it a few characters at a time.
            for (let end = 1; end <= full.length; end += 7) {
                answer.text = full.slice(0, end);
                waitForRendering(answer);
                const items = blockItems();
                if (items.length >= 2 && firstItem === null) {
                    firstItem = items[0];
                    firstGeometry = [items[0].y, items[0].height, items[0].item.text];
                }
                if (firstItem !== null) {
                    verify(items[0] === firstItem, "the first block keeps its item");
                    compare([items[0].y, items[0].height, items[0].item.text], firstGeometry, "and doesn't move or change");
                }
                verify(answer.height >= lastHeight - 0.5, "the card never shrinks while streaming");
                lastHeight = answer.height;
            }
            verify(firstItem !== null, "the stream reached a second block");
            answer.text = full;
            answer.streaming = false;
            compare(answer.blockCount, 5);
        }

        // An answer may hold planted text: nothing in it may become markup
        // that loads a resource, and links take the accent colour.
        // covers: DS-03
        // suite: ui
        function test_prose_renders_safely_with_accent_links() {
            const style = {
                "link": "#915500",
                "muted": "#55555e",
                "mono": "JetBrains Mono"
            };
            const html = MarkdownBlocks.toHtml({
                "kind": "paragraph",
                "source": "See ![a cat](http://127.0.0.1:9/leak?q=secret) and <img src=\"http://127.0.0.1:9/x\"> or [run](javascript:alert) but [the wiki](https://wiki.archlinux.org), `a<b` and **bold**"
            }, style);
            verify(html.indexOf("<img") < 0, "no image tags: " + html);
            verify(html.indexOf("leak") < 0, "image URLs are dropped");
            verify(html.indexOf("&lt;img") >= 0, "raw HTML shows as text");
            verify(html.indexOf("javascript:") < 0, "only http(s) and mailto links");
            verify(html.indexOf('<a href="https://wiki.archlinux.org" style="color:#915500') >= 0, "accent links");
            verify(html.indexOf("a&lt;b</span>") >= 0, "code spans are escaped");
            verify(html.indexOf("<b>bold</b>") >= 0);
            const nested = MarkdownBlocks.toHtml({
                "kind": "list",
                "source": "- one\n  - one a\n- two"
            }, style);
            compare(nested.split("<ul").length - 1, 2, "a nested list: " + nested);
        }

        // covers: DS-03
        // suite: ui
        function test_links_activate_on_click_not_hover() {
            answer.openLinks = false;
            answer.text = "See [the wiki](https://wiki.archlinux.org) for more.";
            waitForRendering(answer);
            const prose = blockItems()[0].item;
            const at = prose.getText(0, prose.length).indexOf("the wiki");
            verify(at >= 0);
            const spot = prose.positionToRectangle(at + 2);
            mouseMove(prose, spot.x + 1, spot.y + spot.height / 2);
            wait(50);
            compare(links.count, 0, "hovering opens nothing");
            mouseClick(prose, spot.x + 1, spot.y + spot.height / 2);
            compare(links.count, 1, "a click does");
            compare(links.signalArguments[0][0], "https://wiki.archlinux.org");
            answer.openLinks = true;
        }

        // covers: DS-03
        // suite: ui
        function test_long_answers_follow_the_stream() {
            answer.maxBodyHeight = 120;
            answer.streaming = true;
            let text = "";
            for (let i = 0; i < 12; ++i) {
                text += "Line " + i + " of a long answer.\n\n";
                answer.text = text;
            }
            waitForRendering(answer);
            const flick = findFlickable(answer);
            tryVerify(() => flick.atYEnd, 1000, "the newest text is in view");
            answer.streaming = false;
            answer.maxBodyHeight = Theme.space48 * 9;
        }

        // covers: DS-03
        // suite: ui
        // acceptance: 0.11
        function test_codeblock_copy_puts_text_on_clipboard() {
            waitForRendering(code);
            const copy = findLabelled(code, "Copy");
            verify(copy !== null);
            mouseClick(copy);
            tryVerify(() => {
                pasteTarget.text = "";
                pasteTarget.paste();
                return pasteTarget.text === "sudo paccache -rk2";
            });
            compare(code.language, "bash");
        }

        // covers: DS-03, DS-06
        // suite: ui
        function test_error_keeps_the_partial_answer_and_copy_answer_copies() {
            answer.text = "Your disk is mostly used by:\n\n- Docker images";
            answer.errorText = qsTr("The answer stopped: the connection to Anthropic dropped.");
            waitForRendering(answer);
            compare(answer.blockCount, 2, "partial text kept");
            verify(findLabelled(answer, "Try again").visible);
            verify(!findLabelled(answer, "Copy answer").visible);
            answer.errorText = "";
            waitForRendering(answer);
            mouseClick(findLabelled(answer, "Copy answer"));
            tryVerify(() => {
                pasteTarget.text = "";
                pasteTarget.paste();
                return pasteTarget.text === answer.text;
            });
            compare(answer.Accessible.description, answer.text, "the answer text is exposed");
            compare(audit.problems(scene), []);
        }

        function findFlickable(root) {
            const stack = [root];
            while (stack.length) {
                const item = stack.pop();
                if (item.contentY !== undefined && item.atYEnd !== undefined)
                    return item;
                for (let i = 0; i < item.children.length; ++i)
                    stack.push(item.children[i]);
            }
            return null;
        }

        function blockItems() {
            const found = [];
            const stack = [answer];
            while (stack.length) {
                const item = stack.pop();
                if (item.kind !== undefined && item.sourceComponent !== undefined)
                    found.push(item);
                for (let i = 0; i < item.children.length; ++i)
                    stack.push(item.children[i]);
            }
            return found.sort((a, b) => a.index - b.index);
        }

        function findLabelled(root, label) {
            const stack = [root];
            while (stack.length) {
                const item = stack.pop();
                if (item.text === label && item.clicked !== undefined)
                    return item;
                for (let i = 0; i < item.children.length; ++i)
                    stack.push(item.children[i]);
            }
            return null;
        }
    }
}
