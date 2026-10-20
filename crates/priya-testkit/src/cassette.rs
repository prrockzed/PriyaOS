//! [`Cassette`]: one real HTTP exchange with a model provider, recorded
//! once and replayed through [`ScriptedHttp`] in every test run after.
//!
//! What is kept: the request's method, path and JSON body, and the
//! response's status, a few safe headers and its body, one server-sent
//! event per piece. Request headers are never kept, so `Authorization` and
//! `x-api-key` can't be recorded; a key that shows up in the response body
//! is replaced with `[key]`. The `id` and `created` of each streamed chunk
//! change with every recording, so they are written as `chatcmpl-0` and
//! `0`. [`Cassette::reply_split`] replays the body cut at other places,
//! as a server's packets may be. Each cassette folder needs a
//! `RECORDED.md` ([`crate::fixture`]).

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http_script::{BodyStep, HttpReply, ScriptedHttp};

/// Response headers worth keeping; the rest (dates, cookies, server ids)
/// would only make cassettes differ between recordings.
const KEPT_HEADERS: [&str; 2] = ["content-type", "retry-after"];
/// A recording gives up after this long, rather than hang the test run.
const PATIENCE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cassette {
    pub request: RecordedCall,
    pub response: RecordedReply,
    /// The answer's text, read from the chunks when recorded, for tests
    /// to compare with.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedCall {
    pub method: String,
    /// Below the base URL, such as `/chat/completions`.
    pub path: String,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// The body, one event per piece.
    pub chunks: Vec<String>,
}

impl Cassette {
    /// Sends `body` to `{base_url}{path}` for real and records the
    /// exchange. `key`, if any, is sent as a bearer token and never kept.
    pub async fn record(
        base_url: &str,
        path: &str,
        body: Value,
        key: Option<&str>,
    ) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(PATIENCE)
            .build()
            .map_err(|e| e.to_string())?;
        let mut request = client
            .post(format!("{}{path}", base_url.trim_end_matches('/')))
            .json(&body);
        if let Some(key) = key {
            request = request.bearer_auth(key);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter(|(name, _)| KEPT_HEADERS.contains(&name.as_str()))
            .filter_map(|(name, value)| Some((name.to_string(), value.to_str().ok()?.to_owned())))
            .collect();
        // The whole body at once: a key or a character split between two
        // packets is still found and decoded.
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if let Some(key) = key.filter(|k| !k.is_empty()) {
            text = text.replace(key, "[key]");
        }
        let chunks = events(&text);
        Ok(Self {
            request: RecordedCall {
                method: "POST".into(),
                path: path.to_owned(),
                body,
            },
            text: answer_text(&chunks),
            response: RecordedReply {
                status,
                headers,
                chunks,
            },
        })
    }

    /// Reads `<folder>/<name>.json`; the folder must have a `RECORDED.md`.
    #[track_caller]
    pub fn load(folder: &Path, name: &str) -> Self {
        let path = crate::fixture(folder).join(format!("{name}.json"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("can't read cassette {}: {e}", path.display()));
        serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("cassette {} is damaged: {e}", path.display()))
    }

    /// Writes `<folder>/<name>.json`, pretty, so diffs are readable.
    pub fn save(&self, folder: &Path, name: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(folder)?;
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(folder.join(format!("{name}.json")), text + "\n")
    }

    /// The recorded response, to play from [`ScriptedHttp`].
    pub fn reply(&self) -> HttpReply {
        let pieces = self.response.chunks.iter().map(|c| c.clone().into_bytes());
        self.reply_with(pieces.collect())
    }

    /// The recorded response with its body cut every `every` bytes, even
    /// inside a line or a character, as a slow network may deliver it.
    pub fn reply_split(&self, every: usize) -> HttpReply {
        let body = self.response.chunks.concat().into_bytes();
        let pieces = body.chunks(every.max(1)).map(<[u8]>::to_vec);
        self.reply_with(pieces.collect())
    }

    fn reply_with(&self, pieces: Vec<Vec<u8>>) -> HttpReply {
        HttpReply {
            status: self.response.status,
            headers: self.response.headers.clone(),
            body: pieces.into_iter().map(BodyStep::Send).collect(),
        }
    }

    /// A server that answers once with this cassette.
    pub async fn serve(&self) -> std::io::Result<ScriptedHttp> {
        ScriptedHttp::start(vec![self.reply()]).await
    }
}

/// The body as one piece per server-sent event (ending at a blank line),
/// with each chunk's changing `id` and `created` normalised.
fn events(body: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    for event in body.split_inclusive("\n\n") {
        pieces.push(
            event
                .split_inclusive('\n')
                .map(normalise)
                .collect::<String>(),
        );
    }
    pieces
}

/// A `data:` line with its chunk's `id` and `created` made fixed.
fn normalise(line: &str) -> String {
    let Some(data) = line.strip_prefix("data: ") else {
        return line.to_owned();
    };
    let Ok(mut chunk) = serde_json::from_str::<Value>(data.trim_end()) else {
        return line.to_owned();
    };
    if let Some(fields) = chunk.as_object_mut() {
        if fields.contains_key("id") {
            fields.insert("id".into(), Value::from("chatcmpl-0"));
        }
        if fields.contains_key("created") {
            fields.insert("created".into(), Value::from(0));
        }
    }
    let end = &line[line.trim_end().len()..];
    format!("data: {chunk}{end}")
}

/// The text of a streamed Chat Completions answer: every
/// `choices[0].delta.content`, read without the adapter under test.
fn answer_text(chunks: &[String]) -> String {
    chunks
        .concat()
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|data| serde_json::from_str::<Value>(data.trim()).ok())
        .filter_map(|chunk| {
            chunk["choices"][0]["delta"]["content"]
                .as_str()
                .map(str::to_owned)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn a_cassette_records_without_keys_and_replays_as_recorded() {
        let upstream = ScriptedHttp::start(vec![
            HttpReply::sse_steps(vec![
                BodyStep::Send(br#"data: {"id":"chatcmpl-81","created":179,"choices":[{"delta":{"content":"Hi"}}]}"#.to_vec()),
                BodyStep::Send(b"\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" there \xc3".to_vec()),
                BodyStep::Send(b"\xa9, k-sec".to_vec()),
                BodyStep::Send(b"ret\"}}]}\n\ndata: [DONE]\n\n".to_vec()),
            ])
            .header("set-cookie", "session=1"),
        ])
        .await
        .unwrap();
        let body = serde_json::json!({"model": "m", "stream": true});
        let cassette = Cassette::record(
            &upstream.base_url(),
            "/chat/completions",
            body.clone(),
            Some("k-secret"),
        )
        .await
        .unwrap();
        assert_eq!(
            upstream.requests()[0].header("authorization"),
            Some("Bearer k-secret")
        );
        // A key and a character split between packets are still found.
        assert_eq!(cassette.text, "Hi there é, [key]");
        assert_eq!(cassette.response.chunks.len(), 3, "one piece per event");
        assert!(cassette.response.chunks[0].contains(r#""id":"chatcmpl-0""#));
        assert!(cassette.response.chunks[0].contains(r#""created":0"#));
        assert_eq!(cassette.response.status, 200);
        assert_eq!(
            cassette.response.headers,
            [("content-type".to_owned(), "text/event-stream".to_owned())],
            "only safe headers"
        );
        let saved = serde_json::to_string(&cassette).unwrap();
        assert!(!saved.contains("k-secret"), "{saved}");

        let folder = tempfile::tempdir().unwrap();
        cassette.save(folder.path(), "answer").unwrap();
        std::fs::write(folder.path().join(crate::RECORDED_NOTE), "test\n").unwrap();
        let loaded = Cassette::load(folder.path(), "answer");
        assert_eq!(loaded, cassette);

        let replay = loaded.serve().await.unwrap();
        let text = reqwest::Client::new()
            .post(format!("{}/chat/completions", replay.base_url()))
            .json(&body)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(text, cassette.response.chunks.concat());
        let split = loaded.reply_split(7);
        assert!(split.body.len() > 3);
    }
}
