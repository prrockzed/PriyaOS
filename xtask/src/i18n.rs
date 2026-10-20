//! `cargo xtask ui i18n [--check]` (sub-phase 0.11, DS-05): `lupdate`
//! collects every `qsTr` string of PriyaUI and the shell into
//! `ui/i18n/priya_en.ts`, the file translators start from. `--check` fails
//! when the committed file is missing strings or keeps removed ones. It
//! compares the strings, not the bytes, so lupdate versions that format the
//! file differently agree.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use quick_xml::events::Event;

use crate::cmd::read;
use crate::qml::qt_tool;

pub const TS_FILE: &str = "ui/i18n/priya_en.ts";
/// Folders whose strings are translated (the gallery's sample text isn't).
const SOURCES: [&str; 2] = ["ui/PriyaUI", "ui/shell"];

/// A translatable string: its context (the file's component), text,
/// disambiguation comment (`qsTr("Open", "verb")`) and whether it has
/// plural forms. A change to any of them needs new translations.
pub type Message = (String, String, String, bool);

pub fn run(root: &Path, check: bool) -> Result<()> {
    if !check {
        lupdate(root, &root.join(TS_FILE))?;
        println!("Updated {TS_FILE}.");
        return Ok(());
    }
    let dir = tempfile::tempdir().context("making a temporary folder")?;
    let fresh = dir.path().join("fresh.ts");
    lupdate(root, &fresh)?;
    let committed = match fs::read_to_string(root.join(TS_FILE)) {
        Ok(text) => messages(&text)?,
        Err(_) => BTreeSet::new(),
    };
    let diff = difference(&committed, &messages(&read(&fresh)?)?);
    if diff.is_empty() {
        println!("{TS_FILE} has every UI string.");
        return Ok(());
    }
    eprint!("{diff}");
    bail!("{TS_FILE} is out of date; run `cargo xtask ui i18n` and commit it");
}

fn lupdate(root: &Path, target: &Path) -> Result<()> {
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    }
    let output = Command::new(qt_tool("lupdate"))
        .current_dir(root)
        // No line numbers, so moving code doesn't change the file.
        .args([
            "-locations",
            "none",
            "-no-obsolete",
            "-extensions",
            "qml,js",
        ])
        .args(["-source-language", "en", "-target-language", "en"])
        .args(SOURCES)
        .arg("-ts")
        .arg(target)
        .output()
        .context("starting lupdate (Qt's linguist tools)")?;
    if !output.status.success() {
        bail!(
            "lupdate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Every message in a .ts file.
pub fn messages(ts: &str) -> Result<BTreeSet<Message>> {
    let mut found = BTreeSet::new();
    let mut reader = quick_xml::Reader::from_str(ts);
    let mut path: Vec<String> = Vec::new();
    let mut context = String::new();
    // The message being read: (source, comment, numerus).
    let mut message = (String::new(), String::new(), false);
    // The text of the element being read; entities arrive as separate
    // events, so it is collected until the element ends.
    let mut text = String::new();
    loop {
        match reader.read_event().context("reading a .ts file")? {
            Event::Start(e) => {
                let name = e.name().as_ref().to_owned();
                if name == "message" {
                    let numerus = e
                        .attributes()
                        .flatten()
                        .any(|a| a.key.as_ref() == "numerus" && a.value.as_ref() == "yes");
                    message = (String::new(), String::new(), numerus);
                }
                path.push(name);
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) => text.push_str(&entity(r.as_ref())),
            Event::End(_) => {
                match path.as_slice() {
                    [.., c, name] if c == "context" && name == "name" => {
                        context = std::mem::take(&mut text);
                    }
                    [.., m, source] if m == "message" && source == "source" => {
                        message.0 = std::mem::take(&mut text);
                    }
                    [.., m, comment] if m == "message" && comment == "comment" => {
                        message.1 = std::mem::take(&mut text);
                    }
                    [.., m] if m == "message" => {
                        let (source, comment, numerus) = std::mem::take(&mut message);
                        found.insert((context.clone(), source, comment, numerus));
                    }
                    _ => {}
                }
                path.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(found)
}

/// The text of `&name;`: the five XML entities and numeric references.
fn entity(name: &str) -> String {
    let numeric = name
        .strip_prefix("#x")
        .map(|hex| u32::from_str_radix(hex, 16))
        .or_else(|| name.strip_prefix('#').map(str::parse::<u32>));
    if let Some(Ok(code)) = numeric {
        return char::from_u32(code).map(String::from).unwrap_or_default();
    }
    match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        _ => "",
    }
    .to_owned()
}

/// What the committed file lacks or keeps beyond the fresh one, one line
/// each; empty when they agree.
pub fn difference(committed: &BTreeSet<Message>, fresh: &BTreeSet<Message>) -> String {
    let mut out = String::new();
    let describe = |(context, source, comment, numerus): &Message| {
        let mut text = format!("{context}: \"{source}\"");
        if !comment.is_empty() {
            text.push_str(&format!(" ({comment})"));
        }
        if *numerus {
            text.push_str(" (plural)");
        }
        text
    };
    for message in fresh.difference(committed) {
        let _ = writeln!(out, "  missing: {}", describe(message));
    }
    for message in committed.difference(fresh) {
        let _ = writeln!(out, "  removed from the code: {}", describe(message));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: DS-05
    // suite: unit
    #[test]
    fn ts_messages_are_compared_by_context_and_text() {
        let ts = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE TS>
<TS version="2.1" language="en">
<context>
    <name>Button</name>
    <message>
        <source>Working…</source>
        <translation type="unfinished"></translation>
    </message>
</context>
<context>
    <name>Toast</name>
    <message>
        <source>Paused while you&apos;re here &amp; there</source>
        <translation type="unfinished"></translation>
    </message>
    <message>
        <source>Open&#x2026;</source>
        <comment>verb</comment>
        <translation type="unfinished"></translation>
    </message>
    <message numerus="yes">
        <source>%n file(s)</source>
        <translation type="unfinished"><numerusform></numerusform></translation>
    </message>
</context>
</TS>"#;
        let m = |c: &str, s: &str, comment: &str, numerus: bool| -> Message {
            (c.into(), s.into(), comment.into(), numerus)
        };
        let found = messages(ts).unwrap();
        assert!(found.contains(&m("Button", "Working…", "", false)));
        assert!(found.contains(&m("Toast", "Paused while you're here & there", "", false)));
        assert!(
            found.contains(&m("Toast", "Open…", "verb", false)),
            "{found:?}"
        );
        assert!(found.contains(&m("Toast", "%n file(s)", "", true)));
        assert_eq!(found.len(), 4);

        let mut fresh = found.clone();
        fresh.insert(m("Chip", "Look", "", false));
        fresh.remove(&m("Toast", "Paused while you're here & there", "", false));
        fresh.remove(&m("Toast", "Open…", "verb", false));
        fresh.insert(m("Toast", "Open…", "noun", false));
        let diff = difference(&found, &fresh);
        assert!(diff.contains("missing: Chip: \"Look\""), "{diff}");
        assert!(diff.contains("removed from the code: Toast"), "{diff}");
        assert!(
            diff.contains("missing: Toast: \"Open…\" (noun)"),
            "a new comment counts: {diff}"
        );
        assert!(difference(&found, &found).is_empty());
    }

    // Linux only: the CI image has Qt's linguist tools; the macOS runner
    // has no Qt.
    // covers: DS-05
    // suite: ui
    // acceptance: 0.11
    #[cfg(target_os = "linux")]
    #[test]
    fn i18n_ts_file_up_to_date() {
        run(&crate::cmd::workspace_root().unwrap(), true).unwrap();
    }
}
