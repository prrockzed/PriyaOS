.pragma library

// Every component with a stories file (stories/<name>.stories.qml), in the
// gallery's order. `cargo xtask test ui --shots` checks this list against
// the folder.
const components = ["Icon", "FocusRing", "Button", "IconButton", "TextField", "PromptInput", "Card", "AnswerCard", "Chip", "Toast", "Skeleton", "EmptyState", "ErrorState"];

// "--story Button/hover --theme dark --out x.png" → {story: …, theme: …, out: …}.
// Flags without a value are true.
function parseArgs(argv) {
    const args = {};
    for (let i = 0; i < argv.length; ++i) {
        const arg = argv[i];
        if (!arg.startsWith("--") || arg === "--")
            continue;
        const key = arg.slice(2);
        const next = argv[i + 1];
        if (next !== undefined && !next.startsWith("--")) {
            args[key] = next;
            ++i;
        } else {
            args[key] = true;
        }
    }
    return args;
}
