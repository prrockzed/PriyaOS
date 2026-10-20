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
//
// Prose is made safe to render: an answer may contain text a web page or a
// tool planted, so images (`![…](…)`, `<img>`) become their alt text and
// raw HTML tags are escaped. Rendered, either could fetch a URL and leak
// data. Code blocks are shown as plain text and keep everything.

// Up to 3 spaces of indent; the info string's first word is the language.
const FENCE = /^ {0,3}(```+|~~~+)\s*([^\s`]*).*$/;
// A closing run of # only counts after a space: "# C#" stays "C#".
const HEADING = /^ {0,3}(#{1,6})\s+(.*?)(?:\s+#+)?\s*$/;
const LIST_ITEM = /^\s*([-*+]|\d+[.)])\s+/;
const QUOTE = /^\s*>/;

function split(text) {
    const lines = String(text).replace(/\r\n?/g, "\n").split("\n");
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
            if (closing && closing[2] === "" && /^ {0,3}(```+|~~~+)\s*$/.test(line) && closing[1][0] === current.fence[0] && closing[1].length >= current.fence.length) {
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
                source: safe(heading[2]),
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
        source: block.kind === "code" ? block.lines.join("\n") : safe(block.lines.join("\n")),
        open: block.open
    };
}

// Prose Markdown with nothing that loads a resource: images become their
// alt text and HTML tags are escaped. Code spans are left alone.
function safe(markdown) {
    return markdown.split(/(`+[^`]*`+)/).map((part, i) => {
        if (i % 2 === 1)
            return part;
        return part
            // ![alt](url "title") and ![alt][ref] → alt
            .replace(/!\[([^\]]*)\](\([^)]*\)|\[[^\]]*\])?/g, "$1")
            // <tag …>, </tag>, <!-- … --> → shown as text; autolinks stay.
            .replace(/<(?!(?:https?|mailto):)(?=[A-Za-z\/!?])/g, "\\<");
    }).join("");
}
