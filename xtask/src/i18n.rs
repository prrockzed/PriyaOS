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

/// A translatable string: its context (the file's component) and text.
pub type Message = (String, String);

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

/// Every (context, source) in a .ts file.
pub fn messages(ts: &str) -> Result<BTreeSet<Message>> {
    let mut found = BTreeSet::new();
    let mut reader = quick_xml::Reader::from_str(ts);
    let mut path: Vec<String> = Vec::new();
    let mut context = String::new();
    // The text of the element being read; entities arrive as separate
    // events, so it is collected until the element ends.
    let mut text = String::new();
    loop {
        match reader.read_event().context("reading a .ts file")? {
            Event::Start(e) => {
                path.push(e.name().as_ref().to_owned());
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) => text.push_str(match r.as_ref() {
                "amp" => "&",
                "lt" => "<",
                "gt" => ">",
                "quot" => "\"",
                "apos" => "'",
                _ => "",
            }),
            Event::End(_) => {
                match path.as_slice() {
                    [.., c, name] if c == "context" && name == "name" => {
                        context = std::mem::take(&mut text);
                    }
                    [.., m, source] if m == "message" && source == "source" => {
                        found.insert((context.clone(), std::mem::take(&mut text)));
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

/// What the committed file lacks or keeps beyond the fresh one, one line
/// each; empty when they agree.
pub fn difference(committed: &BTreeSet<Message>, fresh: &BTreeSet<Message>) -> String {
    let mut out = String::new();
    for (context, source) in fresh.difference(committed) {
        let _ = writeln!(out, "  missing: {context}: \"{source}\"");
    }
    for (context, source) in committed.difference(fresh) {
        let _ = writeln!(out, "  removed from the code: {context}: \"{source}\"");
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
</context>
</TS>"#;
        let found = messages(ts).unwrap();
        assert!(found.contains(&("Button".into(), "Working…".into())));
        assert!(found.contains(&("Toast".into(), "Paused while you're here & there".into())));
        assert_eq!(found.len(), 2);

        let mut fresh = found.clone();
        fresh.insert(("Chip".into(), "Look".into()));
        fresh.remove(&("Toast".into(), "Paused while you're here & there".into()));
        let diff = difference(&found, &fresh);
        assert!(diff.contains("missing: Chip: \"Look\""), "{diff}");
        assert!(diff.contains("removed from the code: Toast"), "{diff}");
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
