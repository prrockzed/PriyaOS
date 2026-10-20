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
// toHtml() renders a prose block as Qt rich text. It escapes every
// character itself, so nothing in an answer (which may hold text a web page
// or a tool planted) becomes markup: images show their alt text and HTML
// shows as text, so rendering can never fetch a URL and leak data. It also
// lets links take the accent colour, which Qt's Markdown importer can't.

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

// A prose block as Qt rich text. `style` gives the colours and fonts:
// {link, muted, mono}.
function toHtml(block, style) {
    const lineHeight = block.kind === "heading" ? 125 : 145;
    const p = body => '<p style="margin:0; line-height:' + lineHeight + '%">' + body + "</p>";
    if (block.kind === "heading")
        return p(inline(block.source, style));
    if (block.kind === "quote") {
        const text = block.source.split("\n").map(l => l.replace(/^\s*>\s?/, "")).join(" ");
        return '<p style="margin:0 0 0 12px; line-height:145%; color:' + style.muted + '">' + inline(text, style) + "</p>";
    }
    if (block.kind === "list")
        return list(block.source, style, lineHeight);
    return p(inline(block.source.split("\n").join(" "), style));
}

function escapeHtml(text) {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

// Links Priya may open; anything else shows as its label.
function safeUrl(url) {
    return /^(https?:\/\/|mailto:)/i.test(url);
}

// Inline Markdown: code spans, images (alt text), links, autolinks, bold,
// italic and strikethrough.
function inline(text, style) {
    return text.split(/(`[^`]*`)/).map((part, i) => {
        if (i % 2 === 1)
            return '<span style="font-family:\'' + style.mono + '\'">' + escapeHtml(part.slice(1, -1)) + "</span>";
        const link = (label, url) => safeUrl(url) ? '<a href="' + url + '" style="color:' + style.link + '; text-decoration:underline">' + label + "</a>" : label;
        return escapeHtml(part)
            .replace(/!\[([^\]]*)\](\([^)]*\)|\[[^\]]*\])?/g, "$1")
            .replace(/\[([^\]]+)\]\(([^)\s]+)(?:\s+&quot;[^)]*&quot;)?\)/g, (m, label, url) => link(label, url))
            .replace(/&lt;((?:https?|mailto):[^\s&]+)&gt;/g, (m, url) => link(url, url))
            .replace(/\*\*(?=\S)(.+?)\*\*|__(?=\S)(.+?)__/g, (m, a, b) => "<b>" + (a ?? b) + "</b>")
            .replace(/(^|[^*\w])\*(?=\S)([^*]+?)\*(?![*\w])/g, "$1<i>$2</i>")
            .replace(/(^|[^_\w])_(?=\S)([^_]+?)_(?![_\w])/g, "$1<i>$2</i>")
            .replace(/~~(?=\S)(.+?)~~/g, "<s>$1</s>");
    }).join("");
}

// A list block, nested by indentation, as <ul>/<ol>.
function list(source, style, lineHeight) {
    const items = [];
    for (const line of source.split("\n")) {
        const marker = /^(\s*)([-*+]|(\d+)[.)])\s+(.*)$/.exec(line);
        if (marker)
            items.push({
                "indent": marker[1].replace(/\t/g, "    ").length,
                "ordered": marker[3] !== undefined,
                "start": marker[3] !== undefined ? parseInt(marker[3]) : 1,
                "text": marker[4]
            });
        else if (items.length > 0)
            items[items.length - 1].text += " " + line.trim();
    }
    let html = "";
    const open = []; // [{indent, tag}]
    for (const item of items) {
        while (open.length > 0 && item.indent < open[open.length - 1].indent)
            html += "</" + open.pop().tag + ">";
        if (open.length === 0 || item.indent > open[open.length - 1].indent) {
            const tag = item.ordered ? "ol" : "ul";
            const start = item.ordered && item.start !== 1 ? ' start="' + item.start + '"' : "";
            html += "<" + tag + start + ' style="margin:0; -qt-list-indent:1">';
            open.push({
                "indent": item.indent,
                "tag": tag
            });
        }
        html += '<li style="margin:0; line-height:' + lineHeight + '%">' + inline(item.text, style) + "</li>";
    }
    while (open.length > 0)
        html += "</" + open.pop().tag + ">";
    return html;
}
