//! `cargo xtask fixtures check` (sub-phase 0.4, QA-04): recorded test data
//! must say how it was recorded and must not contain secrets.
//!
//! Fixture folders are `tests/fixtures/` inside `crates/` and `modules/`.
//! Every file in one needs a `RECORDED.md` in its own folder or a folder
//! above it (up to `fixtures/`). Every text file is searched for API keys,
//! credential headers with real values, JWTs and private keys. Findings
//! name the file, line and kind, never the secret itself.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::cmd::{files_under, relative};

const RECORDED_NOTE: &str = "RECORDED.md";
const SEARCHED: [&str; 2] = ["crates", "modules"];

pub fn check(root: &Path) -> Result<()> {
    let problems = problems(root)?;
    if problems.is_empty() {
        println!("Fixtures: every folder is documented and nothing looks like a secret.");
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  {problem}");
    }
    bail!("{} fixture problem(s)", problems.len());
}

fn problems(root: &Path) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    let searched: Vec<PathBuf> = SEARCHED.iter().map(|d| root.join(d)).collect();
    for fixtures in fixture_roots(&files_under(&searched, &["target"])?) {
        for file in files_under(std::slice::from_ref(&fixtures), &[])? {
            let rel = relative(root, &file);
            if !documented(&file, &fixtures) {
                problems.push(format!(
                    "{rel}: no {RECORDED_NOTE} in its folder or above it in tests/fixtures"
                ));
            }
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue; // binary data (images, audio) isn't searched
            };
            for (line, kind) in secrets_in(&text) {
                problems.push(format!("{rel}:{line}: looks like {kind}"));
            }
        }
    }
    problems.sort();
    problems.dedup();
    Ok(problems)
}

/// The `tests/fixtures` folders holding any of `files`.
fn fixture_roots(files: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = files
        .iter()
        .filter_map(|f| {
            f.ancestors()
                .find(|a| a.ends_with("tests/fixtures"))
                .map(Path::to_path_buf)
        })
        .collect();
    roots.sort();
    roots.dedup();
    roots
}

fn documented(file: &Path, fixtures: &Path) -> bool {
    file.ancestors()
        .skip(1)
        .take_while(|dir| dir.starts_with(fixtures))
        .any(|dir| dir.join(RECORDED_NOTE).is_file())
}

/// `(line number, kind)` for each line that looks like it holds a secret.
fn secrets_in(text: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let kinds = [
            (has_api_key(line), "an API key (sk-…)"),
            (
                has_credential_header(line),
                "a credential header with a real value",
            ),
            (has_jwt(line), "a JWT"),
            (
                line.contains("-----BEGIN") && line.contains("PRIVATE KEY-----"),
                "a private key",
            ),
        ];
        found.extend(
            kinds
                .into_iter()
                .filter(|(hit, _)| *hit)
                .map(|(_, kind)| (i + 1, kind)),
        );
    }
    found
}

/// Anthropic (`sk-ant-…`) and OpenAI (`sk-…`, `sk-proj-…`) keys: `sk-`
/// followed by at least 20 key characters.
fn has_api_key(line: &str) -> bool {
    line.match_indices("sk-").any(|(i, _)| {
        let starts_word = line[..i]
            .chars()
            .last()
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        starts_word && key_chars(&line[i + 3..]) >= 20
    })
}

/// `x-api-key` or `Authorization` (any case) followed by a value that
/// isn't an obvious placeholder.
fn has_credential_header(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    ["x-api-key", "authorization"].iter().any(|name| {
        lower.match_indices(name).any(|(i, _)| {
            // A header needs a separator after the name: `: `, `": "` or `=`.
            // Without one it is prose ("the authorization flow").
            let after = line[i + name.len()..]
                .trim_start_matches(['"', '\''])
                .trim_start();
            let Some(value) = after.strip_prefix(':').or_else(|| after.strip_prefix('=')) else {
                return false;
            };
            let value = value.trim_start().trim_start_matches(['"', '\'']);
            let value = value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("Basic "))
                .unwrap_or(value);
            let token: String = value
                .chars()
                .take_while(|c| !matches!(c, '"' | '\'' | ',' | ' ' | '}' | ';'))
                .collect();
            !token.is_empty() && !is_placeholder(&token)
        })
    })
}

fn is_placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.starts_with('<')
        || value.starts_with('$')
        || value.chars().all(|c| c == '*' || c == 'x' || c == 'X')
        || ["redact", "placeholder", "dummy", "fake", "test", "example"]
            .iter()
            .any(|word| lower.contains(word))
}

/// Three base64url segments, the first starting `eyJ` (`{"` encoded).
fn has_jwt(line: &str) -> bool {
    line.match_indices("eyJ").any(|(i, _)| {
        let parts: Vec<&str> = line[i..].splitn(3, '.').collect();
        parts.len() == 3
            && key_chars(parts[0]) >= 10
            && key_chars(parts[1]) >= 10
            && key_chars(parts[2]) >= 10
    })
}

/// How many characters at the start of `s` could belong to a key or token.
fn key_chars(s: &str) -> usize {
    s.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .count()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Key-shaped strings are built at run time so this file never holds
    /// one (the secret scanners read it too).
    fn fake_anthropic_key() -> String {
        format!("sk-{}-{}", "ant", "q7Zp2Lx9Wm4Rt8Kv".repeat(3))
    }

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        dir
    }

    // covers: QA-04
    // suite: unit
    // acceptance: 0.4
    #[test]
    fn fixtures_check_finds_api_key_and_auth_header() {
        let key = fake_anthropic_key();
        let jwt = format!(
            "ey{}.ey{}.{}",
            "JhbGciOiJIUzI1NiJ9", "JzdWIiOiIxMjM0NTY3ODkwIn0", "abcdefghijklmnop"
        );
        let pem = format!("-----BEGIN RSA {}-----", "PRIVATE KEY");
        let cassette = format!(
            "{{\"x-api-key\": \"{key}\"}}\nAuthorization: Bearer abcdef123456\n{jwt}\n{pem}\nx-api-key: REDACTED\nAuthorization: Bearer <token>\n"
        );
        let dir = tree(&[
            (
                "crates/priya-ai/tests/fixtures/claude/RECORDED.md",
                "Recorded with priya 0.1.\n",
            ),
            (
                "crates/priya-ai/tests/fixtures/claude/stream.json",
                &cassette,
            ),
        ]);
        assert_eq!(
            problems(dir.path()).unwrap(),
            [
                "crates/priya-ai/tests/fixtures/claude/stream.json:1: looks like a credential header with a real value",
                "crates/priya-ai/tests/fixtures/claude/stream.json:1: looks like an API key (sk-…)",
                "crates/priya-ai/tests/fixtures/claude/stream.json:2: looks like a credential header with a real value",
                "crates/priya-ai/tests/fixtures/claude/stream.json:3: looks like a JWT",
                "crates/priya-ai/tests/fixtures/claude/stream.json:4: looks like a private key",
            ]
        );
    }

    // covers: QA-04
    // suite: unit
    #[test]
    fn fixture_files_need_a_recorded_note_in_their_folder_or_above() {
        let dir = tree(&[
            ("crates/a/tests/fixtures/RECORDED.md", "x"),
            ("crates/a/tests/fixtures/deep/ok.txt", "fine"),
            ("modules/m/tests/fixtures/undocumented/data.txt", "fine"),
            (
                "crates/a/src/not_a_fixture.txt",
                "sk-not-checked-here-because-not-a-fixture",
            ),
        ]);
        assert_eq!(
            problems(dir.path()).unwrap(),
            [
                "modules/m/tests/fixtures/undocumented/data.txt: no RECORDED.md in its folder or above it in tests/fixtures"
            ]
        );
    }

    // covers: QA-04
    // suite: unit
    #[test]
    fn ordinary_text_is_not_mistaken_for_secrets() {
        for line in [
            "task-runner and disk-usage",
            "Authorization: Bearer REDACTED",
            "the authorization flow is described below",
            "\"authorization\": \"<redacted>\"",
            "x-api-key: ${ANTHROPIC_API_KEY}",
            "eyJ.short.one",
        ] {
            assert!(secrets_in(line).is_empty(), "{line}");
        }
    }

    // covers: QA-04
    // suite: unit
    #[test]
    fn the_repository_fixtures_pass() {
        let root = crate::cmd::workspace_root().unwrap();
        assert_eq!(problems(&root).unwrap(), Vec::<String>::new());
    }
}
