//! Keys stay in the keyring (05-security §3): with a known fake key in use,
//! even when a provider echoes it back (in an error reply or mid-stream),
//! no error priyad gives, nothing on disk under its folders (settings, the
//! event log) and nothing it logs holds it. (Payloads here are small, so
//! none go to a compressed blob, which this byte search couldn't read.)

mod common;

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use common::machine;
use priya_ai::keys::SecretStore;
use priya_api::methods::ai::AskParams;
use priya_core::secret::Secret;
use priya_core::settings::KeyPath;
use priya_testkit::http_script::{HttpReply, ScriptedHttp};
use serde_json::json;

const KEY: &str = "sk-test-0123456789-NEVER-STORE-ME";

/// Captured log output.
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);

impl Write for Logs {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Every file under `dir` that contains `needle`.
fn files_containing(dir: &Path, needle: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_containing(&path, needle));
        } else if std::fs::read(&path)
            .is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle))
        {
            found.push(path.display().to_string());
        }
    }
    found
}

// covers: AI-21
// suite: security
// acceptance: 0.12
#[tokio::test]
async fn api_key_never_in_settings_events_or_logs() {
    let logs = Logs::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer({
            let logs = logs.clone();
            move || logs.clone()
        })
        .finish();
    // Global, so logs from every thread are caught; this file has one test.
    tracing::subscriber::set_global_default(subscriber).expect("the only subscriber");

    let m = machine().await;
    m.keys
        .set("gemini", Secret::new(KEY.to_owned()))
        .await
        .unwrap();
    // A provider that echoes the key in its errors, as some proxies do.
    let echo = format!(r#"{{"error":{{"message":"API key not valid: {KEY}"}}}}"#);
    let server = ScriptedHttp::start(vec![
        HttpReply::status(400, &echo),
        HttpReply::status(500, &echo),
        HttpReply::status(401, &echo),
        HttpReply::sse(&[&echo]),
    ])
    .await
    .unwrap();
    for (key, value) in [
        (
            "ai.providers.gemini",
            json!({"kind": "openai_compatible", "base_url": server.base_url()}),
        ),
        ("ai.strategy", json!("fixed:gemini/gemini-3.8-flash")),
    ] {
        m.daemon
            .settings
            .set(&KeyPath::parse(key).unwrap(), &value)
            .await
            .unwrap();
    }
    let ai = m.daemon.settings.handle().current().ai.clone();
    let ask = || async {
        let asked = AskParams {
            prompt: "hi".into(),
            role: None,
        };
        let error = priyad::ai::ask_whole(
            &m.daemon.ai,
            &m.daemon.intents,
            ai.clone(),
            asked,
            "test".into(),
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains(KEY), "{error}");
        let detail = error.detail.clone().unwrap_or_default();
        assert!(!detail.contains(KEY), "{detail}");
        detail
    };
    let details = [ask().await, ask().await];
    assert!(
        details.iter().any(|d| d.contains("[key]")),
        "the echoed key reached the detail, redacted: {details:?}"
    );
    // The health check, which doctor and `priya ai status` use.
    let status = priyad::ai::status::status(&m.daemon.ai, &ai, true).await;
    assert!(!status.problem.unwrap_or_default().contains(KEY));
    // An error line in the middle of a stream.
    ask().await;

    // The key was really used.
    let sent = server.requests();
    assert_eq!(sent.len(), 4);
    assert!(
        sent.iter()
            .all(|r| r.header("authorization") == Some(&*format!("Bearer {KEY}")))
    );
    // Nothing kept it: no file under priyad's folders, no log line.
    assert_eq!(
        files_containing(m.root(), KEY.as_bytes()),
        Vec::<String>::new()
    );
    let logged = String::from_utf8_lossy(&logs.0.lock().unwrap()).into_owned();
    assert!(!logged.is_empty(), "the logs were captured");
    assert!(!logged.contains(KEY), "{logged}");
}
