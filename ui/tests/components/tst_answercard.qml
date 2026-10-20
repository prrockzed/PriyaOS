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
