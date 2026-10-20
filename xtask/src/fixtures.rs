//! `cargo xtask fixtures check` (sub-phase 0.4, QA-04): recorded test data
//! must say how it was recorded and must not contain secrets.
//!
//! Fixture folders are `tests/fixtures/` inside `crates/` and `modules/`.
//! Every file in one needs a `RECORDED.md` in its own folder or a folder
//! above it (up to `fixtures/`). Every text file is searched for API keys,
//! credential headers with real values, JWTs and private keys. Findings
//! name the file, line and kind, never the secret itself.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

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
            if file.is_dir() {
                continue; // a symlink to a folder; its files are listed under it
            }
            let rel = relative(root, &file);
            if !documented(&file, &fixtures) {
                problems.push(format!(
                    "{rel}: no {RECORDED_NOTE} in its folder or above it in tests/fixtures"
                ));
            }
            // Every file is searched, binary or not: a key is ASCII even
            // inside a database file or a Latin-1 text.
            let bytes = std::fs::read(&file).with_context(|| format!("reading {rel}"))?;
            let text = String::from_utf8_lossy(&bytes);
            // RECORDED.md is prose about headers, not a recording of them.
            let is_note = file.ends_with(RECORDED_NOTE);
            for (line, kind) in secrets_in(&text, !is_note) {
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

/// Header names whose values are credentials.
const CREDENTIAL_HEADERS: [&str; 5] = [
    "proxy-authorization",
    "authorization",
    "x-goog-api-key",
    "x-api-key",
    "api-key",
];
/// Words that mark a value as a stand-in, when they are the whole value or
/// one of its `-`/`_`-separated parts.
const PLACEHOLDER_WORDS: [&str; 8] = [
    "redacted",
    "placeholder",
    "dummy",
    "fake",
    "test",
    "example",
    "secret",
    "token",
];

/// `(line number, kind)` for each line that looks like it holds a secret.
/// With `headers`, credential headers are checked too.
fn secrets_in(text: &str, headers: bool) -> Vec<(usize, &'static str)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let kinds = [
            (has_api_key(line), "an API key (sk-…)"),
            (
                headers && has_credential_header(&lines, i),
                "a credential header with a real value",
            ),
            (has_jwt(line), "a JWT"),
            (
                line.contains("-----BEGIN") && line.contains("PRIVATE KEY"),
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
/// followed by at least 20 key characters that look random (a digit and a
/// letter), so words like `sk-learn-compatible-estimators` don't count,
/// and that aren't placeholders (`sk-test-…`).
/// No word boundary is needed, so `%20sk-…` and `\nsk-…` are caught.
fn has_api_key(line: &str) -> bool {
    line.match_indices("sk-").any(|(i, _)| {
        let body: String = line[i + 3..]
            .chars()
            .take_while(|c| is_key_char(*c))
            .collect();
        body.len() >= 20 && looks_random(&body) && !is_placeholder(&body)
    })
}

/// A credential header (any case) whose value looks real. The value may sit
/// on the same line in any common recording format (`Authorization: …`,
/// `"x-api-key": "…"`, `["authorization", "…"]`, HAR's
/// `"name": "Authorization", "value": "…"`) or, when the line has nothing
/// after the name, on the next non-empty line (YAML `- Bearer …`).
fn has_credential_header(lines: &[&str], i: usize) -> bool {
    let lower = lines[i].to_ascii_lowercase();
    CREDENTIAL_HEADERS.iter().any(|name| {
        lower.match_indices(name).any(|(at, _)| {
            let after = &lines[i][at + name.len()..];
            let mut same_line = tokens(after).peekable();
            if same_line.peek().is_none() {
                return lines[i + 1..]
                    .iter()
                    .find(|l| !l.trim().is_empty())
                    .is_some_and(|next| tokens_hold_secret(tokens(next)));
            }
            tokens_hold_secret(same_line)
        })
    })
}

/// The first few tokens after a header name: enough to pass a JSON key
/// such as `value` and an auth scheme such as `Bearer`.
fn tokens_hold_secret<'a>(mut tokens: impl Iterator<Item = &'a str>) -> bool {
    tokens.by_ref().take(4).any(is_real_credential)
}

/// Runs of key characters in `s`.
fn tokens(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !is_key_char(c)).filter(|t| !t.is_empty())
}

/// At least 8 characters, a digit and a letter, and no placeholder word.
fn is_real_credential(token: &str) -> bool {
    token.len() >= 8 && looks_random(token) && !is_placeholder(token)
}

fn is_placeholder(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.chars().all(|c| c == '*' || c == 'x')
        || lower
            .split(['-', '_'])
            .any(|part| PLACEHOLDER_WORDS.contains(&part))
}

fn looks_random(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit()) && s.chars().any(|c| c.is_ascii_alphabetic())
}

/// Three base64url segments, the first starting `eyJ` (`{"` encoded).
fn has_jwt(line: &str) -> bool {
    line.match_indices("eyJ").any(|(i, _)| {
        let parts: Vec<&str> = line[i..].splitn(3, '.').collect();
        parts.len() == 3
            && parts
                .iter()
                .all(|p| p.chars().take_while(|c| is_key_char(*c)).count() >= 10)
    })
}

/// A character that can be part of a key or token.
fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
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
    // regression: 0.4
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
    fn credentials_are_found_in_common_recording_formats() {
        // Built at run time: a literal would trip the secret scanners.
        let token = format!("abc{}ghiJKL", "DEF123456");
        let header_hits = |text: &str| -> Vec<usize> {
            secrets_in(text, true)
                .into_iter()
                .filter(|(_, kind)| kind.contains("header"))
                .map(|(line, _)| line)
                .collect()
        };
        // YAML (vcr): the value on the next line.
        assert_eq!(
            header_hits(&format!(
                "headers:\n  authorization:\n\n    - Bearer {token}\n"
            )),
            [2]
        );
        // JSON array pairs and HAR name/value objects.
        assert_eq!(
            header_hits(&format!("[\"authorization\", \"Bearer {token}\"]")),
            [1]
        );
        assert_eq!(
            header_hits(&format!(
                "{{\"name\": \"Authorization\", \"value\": \"Bearer {token}\"}}"
            )),
            [1]
        );
        // Any auth scheme, any case.
        assert_eq!(header_hits(&format!("authorization: token {token}")), [1]);
        assert_eq!(header_hits(&format!("x-goog-api-key={token}")), [1]);
    }

    // covers: QA-04
    // suite: unit
    #[test]
    fn keys_hidden_by_encoding_or_bad_bytes_are_still_found() {
        let key = fake_anthropic_key();
        assert!(!secrets_in(&format!("Bearer%20{key}"), true).is_empty());
        assert!(!secrets_in(&format!("\"line\\n{key}\""), true).is_empty());

        // A file that isn't valid UTF-8 is still searched.
        let mut bytes = b"na\xefve ".to_vec(); // Latin-1, not UTF-8
        bytes.extend_from_slice(key.as_bytes());
        let dir = tree(&[("crates/a/tests/fixtures/RECORDED.md", "x")]);
        fs::write(dir.path().join("crates/a/tests/fixtures/latin1.txt"), bytes).unwrap();
        assert_eq!(
            problems(dir.path()).unwrap(),
            ["crates/a/tests/fixtures/latin1.txt:1: looks like an API key (sk-…)"]
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
            "sk-learn-compatible-estimators-pack",
            "Authorization: bearer REDACTED",
            "Authorization: Token REDACTED",
            "Authorization: Bearer",
            "authorization: Bearer sk-test-abc123def456ghi",
            "x-api-key: test-key-1234",
            "Authorization: Bearer REDACTED",
            "the authorization flow is described below",
            "\"authorization\": \"<redacted>\"",
            "x-api-key: ${ANTHROPIC_API_KEY}",
            "eyJ.short.one",
        ] {
            assert!(secrets_in(line, true).is_empty(), "{line}");
        }
    }

    // covers: QA-04
    // suite: unit
    #[test]
    fn recorded_notes_may_talk_about_headers() {
        let dir = tree(&[(
            "crates/a/tests/fixtures/RECORDED.md",
            "The Authorization: header was removed before saving (curl 8.11).\n",
        )]);
        assert!(problems(dir.path()).unwrap().is_empty());
    }

    // covers: QA-04
    // suite: unit
    #[test]
    fn the_repository_fixtures_pass() {
        let root = crate::cmd::workspace_root().unwrap();
        assert_eq!(problems(&root).unwrap(), Vec::<String>::new());
    }
}
