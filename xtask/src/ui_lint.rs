//! `cargo xtask ui lint` (sub-phase 0.11, DS-05, DS-06, QA-09): rules that
//! keep PriyaUI, the shell and the gallery on tokens, translatable and
//! readable by screen readers (11 §6–8).
//!
//! - **Tokens only.** No colour literals (`"#…"`, a named colour on a
//!   `…color` property, `Qt.rgba(`, `rgba(`, …) and no numeric
//!   `pixelSize`/`pointSize` outside the generated `Theme.qml`.
//! - **Translatable.** A string with letters in `text:`,
//!   `placeholderText:`, `Accessible.name:`, `Accessible.description:` or
//!   `ToolTip.text:` must be inside `qsTr(…)`. Gallery stories hold fixed
//!   sample text, so this rule skips `ui/gallery/`.
//! - **Named.** An interactive item (`MouseArea`, `TapHandler`, Qt Quick
//!   Controls such as `T.Button`, `TextInput`, `TextEdit`, anything with
//!   `activeFocusOnTab: true`) needs `Accessible.name` on itself or an
//!   enclosing object in the same file. PriyaUI controls carry their name in
//!   a property: `IconButton` needs `accessibleName`, `Button` and `Chip`
//!   need `text` (a Chip may set `mode` instead), `TextField` needs `label`.
//!
//! The scan is line- and brace-based, not a QML parser: comments are
//! dropped, strings are kept apart, and a `Type {` opens an object.

use std::path::Path;

use anyhow::{Result, bail};

use crate::cmd::{files_under, read, relative};

/// Folders linted, relative to the repository root.
const ROOTS: [&str; 3] = ["ui/PriyaUI", "ui/shell", "ui/gallery"];
/// Generated from the tokens; it is where colour values belong.
const THEME_QML: &str = "ui/PriyaUI/Theme.qml";
/// Stories show fixed sample text, which needs no translation.
const UNTRANSLATED_OK: &str = "ui/gallery/";

/// Properties whose text a person reads or hears (`label`, `title` and
/// `body` are PriyaUI's TextField, EmptyState and ErrorState texts).
const TEXT_PROPERTIES: [&str; 8] = [
    "text",
    "placeholderText",
    "Accessible.name",
    "Accessible.description",
    "ToolTip.text",
    "label",
    "title",
    "body",
];
/// Calls that make colours from numbers.
const COLOUR_CALLS: [&str; 6] = [
    "Qt.rgba(", "Qt.hsla(", "Qt.hsva(", "Qt.hsl(", "rgba(", "rgb(",
];
/// Qt types a user acts on. Qt Quick Controls only count with a namespace
/// (`T.Button`), so PriyaUI's own `Button` is checked by its rule instead.
const INTERACTIVE: [&str; 4] = ["MouseArea", "TapHandler", "TextInput", "TextEdit"];
const CONTROLS: [&str; 11] = [
    "AbstractButton",
    "Button",
    "ToolButton",
    "CheckBox",
    "Switch",
    "RadioButton",
    "TextField",
    "TextArea",
    "ComboBox",
    "Slider",
    "Control",
];
/// PriyaUI controls and the properties that give them a spoken name.
const NAMED_BY: [(&str, &[&str]); 4] = [
    ("IconButton", &["accessibleName"]),
    ("Button", &["text", "Accessible.name"]),
    // A mode chip names itself ("Sandbox").
    ("Chip", &["text", "mode", "Accessible.name"]),
    ("TextField", &["label", "Accessible.name"]),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

/// Lints every QML and JS file under the linted folders.
pub fn check(root: &Path) -> Result<()> {
    let dirs: Vec<_> = ROOTS.iter().map(|r| root.join(r)).collect();
    let mut problems = Vec::new();
    let mut files = 0;
    for path in files_under(&dirs, &["fixtures"])? {
        let rel = relative(root, &path);
        if !(rel.ends_with(".qml") || rel.ends_with(".js")) {
            continue;
        }
        files += 1;
        problems.extend(lint(&read(&path)?, &rel));
    }
    if problems.is_empty() {
        println!("UI lint: {files} files, no problems.");
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  {problem}");
    }
    bail!("{} UI lint problem(s)", problems.len());
}

/// The problems in one file; `file` is its repository-relative path, which
/// decides which rules apply.
pub fn lint(source: &str, file: &str) -> Vec<Problem> {
    let scan = Scan::new(source);
    let mut problems = Vec::new();
    let mut add = |line: usize, message: String| {
        problems.push(Problem {
            file: file.to_owned(),
            line,
            message,
        });
    };

    if file != THEME_QML {
        for lit in &scan.literals {
            if is_hex_colour(&lit.text) {
                add(
                    lit.line,
                    format!("colour literal \"{}\"; use a Theme colour", lit.text),
                );
            } else if names_colour(&lit.before) && is_word(&lit.text) && lit.text != "transparent" {
                add(
                    lit.line,
                    format!("named colour \"{}\"; use a Theme colour", lit.text),
                );
            }
        }
        for (i, line) in scan.blank_lines().enumerate() {
            if let Some(call) = COLOUR_CALLS.iter().find(|c| has_call(line, c)) {
                add(
                    i + 1,
                    format!("`{call}…)` makes a colour; use a Theme colour"),
                );
            }
            if numeric_font_size(line) {
                add(
                    i + 1,
                    "numeric font size; use a Theme type style".to_owned(),
                );
            }
        }
    }

    let (objects, code_blocks) = if file.ends_with(".qml") {
        objects(&scan.blank)
    } else {
        (Vec::new(), Vec::new())
    };

    // Bindings only: a `text:` key in a JavaScript object isn't shown.
    if file.ends_with(".qml") && !file.starts_with(UNTRANSLATED_OK) {
        let in_code = |at: usize| code_blocks.iter().any(|r| r.contains(&at));
        for lit in scan.literals.iter().filter(|l| !in_code(l.offset)) {
            // `T.ToolTip.text` and `label.text` count too.
            let reads_out = binding(&lit.before).is_some_and(|b| {
                TEXT_PROPERTIES
                    .iter()
                    .any(|t| b.property == *t || b.property.ends_with(&format!(".{t}")))
                    && is_value(b.expression)
            });
            if reads_out && lit.text.chars().any(char::is_alphabetic) {
                add(
                    lit.line,
                    format!("\"{}\" isn't translatable; wrap it in qsTr()", lit.text),
                );
            }
        }
    }

    for (i, object) in objects.iter().enumerate() {
        if let Some(message) = unnamed(&objects, i) {
            add(object.line, message);
        }
    }
    problems.sort_by_key(|p| p.line);
    problems
}

/// A string literal and what precedes it.
struct Literal {
    line: usize,
    /// Byte offset of its opening quote in `Scan::blank`.
    offset: usize,
    text: String,
    /// Code before it on its line, other strings blanked: it names the
    /// property (`text: x ? "a" : "b"`) and shows a `qsTr(` call.
    before: String,
}

/// A source file with comments removed and strings set apart.
struct Scan {
    /// The source with comments and string contents replaced by spaces
    /// (quotes and newlines kept), so braces and calls inside them don't
    /// count.
    blank: String,
    literals: Vec<Literal>,
}

impl Scan {
    fn new(source: &str) -> Self {
        let chars: Vec<char> = source.chars().collect();
        let mut blank = String::with_capacity(source.len());
        let mut literals = Vec::new();
        let mut line = 1;
        let mut line_start = 0; // index in `blank` where this line starts
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            if c == '/' && next == Some('/') {
                while i < chars.len() && chars[i] != '\n' {
                    blank.push(' ');
                    i += 1;
                }
                continue;
            }
            if c == '/' && next == Some('*') {
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    push_blank(&mut blank, chars[i], &mut line, &mut line_start);
                    i += 1;
                }
                blank.push_str("  ");
                i += 2;
                continue;
            }
            if matches!(c, '"' | '\'' | '`') {
                let start_line = line;
                let before = blank[line_start..].to_owned();
                let offset = blank.len();
                blank.push(c);
                let mut text = String::new();
                i += 1;
                while i < chars.len() && chars[i] != c {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        text.push(chars[i + 1]);
                        blank.push(' ');
                        push_blank(&mut blank, chars[i + 1], &mut line, &mut line_start);
                        i += 2;
                        continue;
                    }
                    // Every string ends at the line's end, so a stray quote
                    // or a backtick in a regex (/^```/) can't swallow the
                    // file. PriyaUI writes no multi-line template strings.
                    if chars[i] == '\n' {
                        break;
                    }
                    text.push(chars[i]);
                    push_blank(&mut blank, chars[i], &mut line, &mut line_start);
                    i += 1;
                }
                if chars.get(i) == Some(&c) {
                    blank.push(c);
                    i += 1;
                }
                literals.push(Literal {
                    line: start_line,
                    offset,
                    text,
                    before,
                });
                continue;
            }
            // Code is kept as it is.
            blank.push(c);
            if c == '\n' {
                line += 1;
                line_start = blank.len();
            }
            i += 1;
        }
        Self { blank, literals }
    }

    fn blank_lines(&self) -> std::str::Lines<'_> {
        self.blank.lines()
    }
}

/// Pushes a space for a character inside a string or comment (or the
/// newline itself) and tracks lines.
fn push_blank(blank: &mut String, c: char, line: &mut usize, line_start: &mut usize) {
    if c == '\n' {
        blank.push('\n');
        *line += 1;
        *line_start = blank.len();
    } else {
        blank.push(' ');
    }
}

/// `#RGB`, `#ARGB`, `#RRGGBB` or `#AARRGGBB`.
fn is_hex_colour(text: &str) -> bool {
    text.strip_prefix('#').is_some_and(|hex| {
        matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit())
    })
}

fn is_word(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_alphabetic())
}

/// The string is a value (not a call argument or comparison operand) of a
/// colour property: `…color:`, `…Color:` or `property color x:`.
fn names_colour(before: &str) -> bool {
    binding(before).is_some_and(|b| {
        (b.property.ends_with("color") || b.property.ends_with("Color") || b.kind == "color")
            && is_value(b.expression)
    })
}

/// `call` appears as a call, not as the end of a longer name.
fn has_call(line: &str, call: &str) -> bool {
    line.match_indices(call).any(|(at, _)| {
        call.starts_with("Qt.")
            || !line[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
    })
}

/// `pixelSize: 14`, `font.pointSize: on ? 20 : 10.5` and the like: a number
/// as the value or as a branch of a conditional. `Theme.body.pixelSize * 2`
/// is allowed.
fn numeric_font_size(line: &str) -> bool {
    let Some(b) = binding(line) else {
        return false;
    };
    let key = b.property.rsplit('.').next().unwrap_or_default();
    if key != "pixelSize" && key != "pointSize" {
        return false;
    }
    let mut after_branch = true; // the start counts as a branch
    for c in b.expression.chars() {
        if c.is_whitespace() {
            continue;
        }
        if after_branch && (c.is_ascii_digit() || c == '.') {
            return true;
        }
        after_branch = c == '?' || c == ':';
    }
    false
}

/// The last statement on a line of code: what it assigns and the code
/// after the colon.
struct Binding<'a> {
    /// `Accessible.name`, or `placeholderText` for
    /// `property string placeholderText: …`.
    property: &'a str,
    /// The type of a `property <type> name:` declaration, else empty.
    kind: &'a str,
    expression: &'a str,
}

fn binding(line: &str) -> Option<Binding<'_>> {
    // The last statement on the line: `Text { text: "a" }`.
    let line = line.rsplit(['{', ';']).next().unwrap_or(line).trim_start();
    let line = line.strip_prefix("required ").unwrap_or(line);
    let line = line.strip_prefix("readonly ").unwrap_or(line);
    let (kind, line) = match line.strip_prefix("property ") {
        Some(rest) => rest.trim_start().split_once(char::is_whitespace)?,
        None => ("", line),
    };
    let (name, expression) = line.split_once(':')?;
    let name = name.trim();
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    ok.then_some(Binding {
        property: name,
        kind,
        expression,
    })
}

/// Whether a string ending `expression` is (part of) the value itself:
/// not an argument of a call (`qsTr("…")`, `formatDateTime(d, "hh:mm")`),
/// an index (`model["title"]`) or a comparison operand (`state === "busy"`).
fn is_value(expression: &str) -> bool {
    let mut depth = 0i32;
    for c in expression.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ => {}
        }
    }
    let end = expression.trim_end();
    depth <= 0 && !["=", "<", ">"].iter().any(|op| end.ends_with(op))
}

/// A `Type { … }` object in a file.
#[derive(Debug)]
struct Object {
    kind: String,
    line: usize,
    /// Properties set directly on it (`Accessible { name: … }` counts as
    /// `Accessible.name`).
    properties: Vec<String>,
    /// The enclosing object in the same file.
    parent: Option<usize>,
}

/// Every object in `blank` (a `Scan::blank`), outermost first, and the
/// byte ranges of JavaScript blocks (function bodies, handlers, object
/// literals).
fn objects(blank: &str) -> (Vec<Object>, Vec<std::ops::Range<usize>>) {
    struct Frame {
        /// Index into `found`, for objects; `None` for other blocks.
        object: Option<usize>,
        /// A grouped property block such as `Accessible {`, `font {`.
        group: Option<String>,
        start: usize,
    }
    let mut code_blocks = Vec::new();
    let mut found: Vec<Object> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut line = 1;
    let mut statement_start = true;
    let mut word = String::new();
    let mut words: Vec<String> = Vec::new(); // since the statement started
    for (at, c) in blank.char_indices() {
        if c.is_alphanumeric() || c == '_' || c == '.' {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        match c {
            '\n' => {
                line += 1;
                statement_start = true;
                words.clear();
            }
            ';' => {
                statement_start = true;
                words.clear();
            }
            ':' if statement_start => {
                // `name:` or `property type name:` at the start of a statement.
                statement_start = false;
                if let (Some(name), Some(frame)) = (words.last(), stack.last()) {
                    let owner = stack.iter().rev().find_map(|f| f.object);
                    let direct = frame.object.is_some() || frame.group.is_some();
                    if let (true, Some(owner)) = (direct, owner) {
                        let name = match &frame.group {
                            Some(group) => format!("{group}.{name}"),
                            None => name.clone(),
                        };
                        found[owner].properties.push(name);
                    }
                }
            }
            '{' => {
                let last = words.last().map(String::as_str).unwrap_or_default();
                let type_name = last.rsplit('.').next().unwrap_or_default();
                let is_type = type_name.chars().next().is_some_and(char::is_uppercase);
                // `Accessible {` is a grouped property, not an object.
                // JavaScript (`if (k === Kind.TextField) {`) holds no objects.
                let in_code = stack
                    .iter()
                    .any(|f| f.object.is_none() && f.group.is_none());
                if is_type && last != "Accessible" && !in_code {
                    found.push(Object {
                        kind: last.to_owned(),
                        line,
                        properties: Vec::new(),
                        parent: stack.iter().rev().find_map(|f| f.object),
                    });
                    stack.push(Frame {
                        object: Some(found.len() - 1),
                        group: None,
                        start: at,
                    });
                } else {
                    let group = (statement_start && words.len() == 1).then(|| last.to_owned());
                    stack.push(Frame {
                        object: None,
                        group,
                        start: at,
                    });
                }
                statement_start = true;
                words.clear();
            }
            '}' => {
                if let Some(Frame {
                    object: None,
                    group: None,
                    start,
                }) = stack.pop()
                {
                    code_blocks.push(start..at);
                }
                statement_start = true;
                words.clear();
            }
            c if c.is_whitespace() || c == '"' || c == '\'' || c == '`' => {}
            _ => statement_start = false,
        }
    }
    (found, code_blocks)
}

impl Object {
    fn has(&self, property: &str) -> bool {
        self.properties.iter().any(|p| p == property)
    }

    fn named_here(&self) -> bool {
        self.has("Accessible.name")
    }
}

/// Why object `i` lacks a spoken name, if it does.
fn unnamed(objects: &[Object], i: usize) -> Option<String> {
    let object = &objects[i];
    let (namespace, kind) = match object.kind.rsplit_once('.') {
        Some((ns, kind)) => (Some(ns), kind),
        None => (None, object.kind.as_str()),
    };
    if namespace.is_none()
        && let Some((_, props)) = NAMED_BY.iter().find(|(k, _)| *k == kind)
    {
        if props.iter().any(|p| object.has(p)) {
            return None;
        }
        return Some(format!(
            "{kind} needs {} so screen readers can name it",
            props
                .iter()
                .map(|p| format!("`{p}`"))
                .collect::<Vec<_>>()
                .join(" or ")
        ));
    }
    let interactive = (INTERACTIVE.contains(&kind)
        || (namespace.is_some() && CONTROLS.contains(&kind))
        || object.has("activeFocusOnTab"))
        && !object.has("Accessible.ignored");
    let named_around = std::iter::successors(Some(object), |o| o.parent.map(|p| &objects[p]))
        .any(Object::named_here);
    (interactive && !named_around).then(|| {
        format!(
            "{} is interactive but has no Accessible.name here or around it",
            object.kind
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let root = crate::cmd::workspace_root().unwrap();
        read(&root.join("ui/tests/lint/fixtures").join(name)).unwrap()
    }

    fn messages(source: &str, file: &str) -> Vec<String> {
        lint(source, file).iter().map(ToString::to_string).collect()
    }

    // covers: QA-09
    // suite: ui
    // acceptance: 0.11
    #[test]
    fn ui_lint_rejects_color_literal() {
        let found = messages(&fixture("BadColours.qml"), "ui/PriyaUI/BadColours.qml");
        let expect = [
            ":7: colour literal \"#FF0000\"",
            ":8: named colour \"white\"",
            ":9: `Qt.rgba(…)`",
            ":10: `rgba(…)`",
            ":11: numeric font size",
            ":13: numeric font size",
            ":14: named colour \"red\"",
            ":15: named colour \"red\"",
            ":16: numeric font size",
        ];
        for e in expect {
            assert!(
                found.iter().any(|f| f.contains(e)),
                "missing {e} in {found:#?}"
            );
        }
        assert_eq!(found.len(), expect.len(), "{found:#?}");
        // The generated theme is where colours live.
        assert!(lint("QtObject { property color a: \"#FFFFFF\" }", THEME_QML).is_empty());
    }

    // covers: DS-05
    // suite: ui
    // acceptance: 0.11
    #[test]
    fn ui_lint_rejects_untranslated_string() {
        let source = fixture("BadStrings.qml");
        let found = messages(&source, "ui/PriyaUI/BadStrings.qml");
        let expect = [
            ":6: \"Keep changes\" isn't translatable",
            ":7: \"Ask anything\" isn't translatable",
            ":8: \"Close\" isn't translatable",
            ":9: \"Copies the answer\" isn't translatable",
            ":10: \"Copy\" isn't translatable",
            ":11: \"Off\" isn't translatable",
            ":12: \"Search\" isn't translatable",
            ":16: \"Provider name\" isn't translatable",
            ":17: \"No tasks\" isn't translatable",
        ];
        for e in expect {
            assert!(
                found.iter().any(|f| f.contains(e)),
                "missing {e} in {found:#?}"
            );
        }
        assert_eq!(found.len(), expect.len(), "{found:#?}");
        // Gallery stories hold fixed sample text.
        assert!(lint(&source, "ui/gallery/stories/X.stories.qml").is_empty());
    }

    // covers: DS-06
    // suite: ui
    // acceptance: 0.11
    #[test]
    fn ui_lint_rejects_missing_accessible_name() {
        let found = messages(&fixture("BadNames.qml"), "ui/shell/BadNames.qml");
        let expect = [
            ":5: MouseArea is interactive",
            ":7: T.Button is interactive",
            ":8: Rectangle is interactive",
            ":12: IconButton needs `accessibleName`",
            ":13: Button needs `text` or `Accessible.name`",
            ":14: TextField needs `label` or `Accessible.name`",
        ];
        for e in expect {
            assert!(
                found.iter().any(|f| f.contains(e)),
                "missing {e} in {found:#?}"
            );
        }
        assert_eq!(found.len(), expect.len(), "{found:#?}");
    }

    // covers: DS-06, QA-09
    // suite: ui
    #[test]
    fn ui_lint_accepts_good_code() {
        let good = fixture("Good.qml");
        assert_eq!(messages(&good, "ui/PriyaUI/Good.qml"), Vec::<String>::new());
    }

    // covers: QA-09
    // suite: unit
    #[test]
    fn ui_lint_passes_on_the_repository() {
        check(&crate::cmd::workspace_root().unwrap()).unwrap();
    }

    // covers: QA-09
    // suite: unit
    #[test]
    fn scanner_drops_comments_and_sets_strings_apart() {
        let scan = Scan::new("a: \"x // y\" // \"z\"\n/* \"w\"\n{ */ b: 'q\\'r'\n");
        let texts: Vec<&str> = scan.literals.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["x // y", "q'r"]);
        assert_eq!(scan.literals[1].line, 3);
        assert!(!scan.blank.contains('{'), "braces in comments are dropped");
        let b = binding("  property string placeholderText: x").unwrap();
        assert_eq!(
            (b.property, b.kind, b.expression),
            ("placeholderText", "string", " x")
        );
        assert_eq!(
            binding("Accessible.name: x").unwrap().property,
            "Accessible.name"
        );
        assert!(binding("x ? a : b").is_none());
        assert!(is_value(" on ? qsTr(\"a\") : "));
        assert!(!is_value(" qsTr("));
        assert!(!is_value(" model["));
        assert!(!is_value(" state === "));
        // An escaped newline keeps the line count; a backtick ends with its line.
        let scan = Scan::new("a: \"x\\\ny\"\nb: /^```/\nc: \"z\"\n");
        let lines: Vec<usize> = scan.literals.iter().map(|l| l.line).collect();
        assert_eq!(lines, [1, 3, 3, 4], "nothing after the regex is swallowed");
    }
}
