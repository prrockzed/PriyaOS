.pragma library

// Splits Markdown into the blocks AnswerCard renders one by one (0.11):
// "paragraph", "heading", "list", "quote" and "code" (fenced). Blank lines
// end blocks, so while an answer streams in, appending text only changes
// the last block or adds new ones: finished blocks never reflow.
//
// Each block is {kind, level, lang, source, open}:
// - source: the block's Markdown (for code, the code itself);
// - level: 1–6 for headings, else 0;
// - lang: a code block's language label, else "";
// - open: a code fence that isn't closed yet (still streaming).

const FENCE = /^\s*(```+|~~~+)\s*([\w+#.-]*)\s*$/;
const HEADING = /^(#{1,6})\s+(.*?)\s*#*\s*$/;
const LIST_ITEM = /^\s*([-*+]|\d+[.)])\s+/;
const QUOTE = /^\s*>/;

function split(text) {
    const lines = String(text).split("\n");
    const blocks = [];
    let current = null;

    function flush() {
        if (current !== null)
            blocks.push(finish(current));
        current = null;
    }

    for (let i = 0; i < lines.length; ++i) {
        const line = lines[i];
        if (current !== null && current.kind === "code") {
            const closing = FENCE.exec(line);
            if (closing && closing[2] === "" && closing[1][0] === current.fence[0] && closing[1].length >= current.fence.length) {
                current.open = false;
                flush();
            } else {
                current.lines.push(line);
            }
            continue;
        }
        const fence = FENCE.exec(line);
        if (fence) {
            flush();
            current = {
                kind: "code",
                fence: fence[1],
                lang: fence[2],
                lines: [],
                open: true
            };
            continue;
        }
        if (line.trim() === "") {
            flush();
            continue;
        }
        const heading = HEADING.exec(line);
        if (heading) {
            flush();
            blocks.push({
                kind: "heading",
                level: heading[1].length,
                lang: "",
                source: heading[2],
                open: false
            });
            continue;
        }
        const kind = LIST_ITEM.test(line) ? "list" : QUOTE.test(line) ? "quote" : "paragraph";
        // Lists and quotes take lazy continuation lines; a paragraph ends
        // where a list or quote starts.
        if (current !== null && (current.kind === kind || (kind === "paragraph" && current.kind !== "paragraph"))) {
            current.lines.push(line);
            continue;
        }
        flush();
        current = {
            kind: kind,
            lines: [line],
            open: false
        };
    }
    flush();
    return blocks;
}

function finish(block) {
    return {
        kind: block.kind,
        level: 0,
        lang: block.lang || "",
        source: block.lines.join("\n"),
        open: block.open
    };
}
