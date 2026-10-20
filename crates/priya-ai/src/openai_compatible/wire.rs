//! The Chat Completions wire format: the request body, and turning each
//! streamed chunk into [`StreamEvent`]s.
//!
//! Servers differ in small ways, which the decoder accepts:
//! - Ollama and OpenAI send usage in a last chunk with no choices; Gemini
//!   may put it next to the finish reason.
//! - The first chunk may carry only the role, with empty content.
//! - Some servers end with `[DONE]` and no finish reason; `[DONE]` says
//!   the answer is whole, so it counts as a normal stop.
//! - Fields may be `null` rather than absent (vLLM, Azure), and local
//!   servers may send an empty model name or finish reason.
//! - Gemini may report usage on several chunks as it counts; each becomes
//!   a `Usage` event and the last one wins.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{FinishReason, ModelRequest, Role, StreamEvent, Usage};

/// A streamed chat request. Serialized straight into the HTTP body, not
/// through `serde_json::Value`, which would turn a temperature of 0.7 into
/// 0.699999988.
#[derive(Serialize)]
pub struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    stream: bool,
    stream_options: StreamOptions,
    /// Not `max_completion_tokens`: Ollama and llama.cpp know only this
    /// name. Gemini 2.5 counts its thinking against it, so a very small
    /// limit can end with `length` and no text.
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct WireMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

/// The body of a streamed chat request.
pub fn request_body(request: &ModelRequest) -> ChatRequest<'_> {
    let system = request.system.as_deref().map(|text| WireMessage {
        role: "system",
        content: text,
    });
    let turns = request.messages.iter().map(|message| WireMessage {
        role: match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        },
        content: &message.text,
    });
    ChatRequest {
        model: &request.model,
        messages: system.into_iter().chain(turns).collect(),
        stream: true,
        stream_options: StreamOptions {
            include_usage: true,
        },
        max_tokens: request.max_output_tokens,
        temperature: request.temperature,
    }
}

// Every field is an `Option`: `#[serde(default)]` alone refuses `null`.
#[derive(Deserialize)]
struct Chunk {
    model: Option<String>,
    choices: Option<Vec<Choice>>,
    usage: Option<WireUsage>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Option<Delta>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct WireUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    prompt_tokens_details: Option<UsageDetails>,
}

#[derive(Deserialize)]
struct UsageDetails {
    cached_tokens: Option<u64>,
}

/// A chunk that can't become events.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChunkError {
    /// Not the JSON a chunk should be.
    #[error("unreadable chunk: {0}")]
    Malformed(String),
    /// The server reported an error inside the stream.
    #[error("error in the stream: {0}")]
    Provider(String),
}

/// Turns one answer's chunks into events.
#[derive(Debug)]
pub struct Decoder {
    requested: String,
    started: bool,
    finished: bool,
}

impl Decoder {
    /// `requested` names the model until a chunk says which one answers.
    pub fn new(requested: &str) -> Self {
        Self {
            requested: requested.to_owned(),
            started: false,
            finished: false,
        }
    }

    pub fn chunk(&mut self, data: &str) -> Result<Vec<StreamEvent>, ChunkError> {
        let chunk: Chunk =
            serde_json::from_str(data).map_err(|e| ChunkError::Malformed(e.to_string()))?;
        if let Some(error) = chunk.error {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .map_or_else(|| error.to_string(), str::to_owned);
            return Err(ChunkError::Provider(message));
        }
        let mut events = Vec::new();
        if !self.started {
            self.started = true;
            let model = chunk.model.filter(|m| !m.is_empty());
            events.push(StreamEvent::Started {
                model: model.unwrap_or_else(|| self.requested.clone()),
            });
        }
        for choice in chunk.choices.unwrap_or_default() {
            let text = choice.delta.and_then(|d| d.content);
            if let Some(text) = text.filter(|t| !t.is_empty()) {
                events.push(StreamEvent::TextDelta(text));
            }
            if let Some(reason) = choice.finish_reason.filter(|r| !r.is_empty()) {
                events.extend(self.finish(FinishReason::from_provider(&reason)));
            }
        }
        if let Some(usage) = chunk.usage {
            let mut total = Usage::new(
                usage.prompt_tokens.unwrap_or_default(),
                usage.completion_tokens.unwrap_or_default(),
            );
            if let Some(cached) = usage.prompt_tokens_details.and_then(|d| d.cached_tokens) {
                total = total.with_cached(cached);
            }
            events.push(StreamEvent::Usage(total));
        }
        Ok(events)
    }

    /// `[DONE]` arrived: the answer is whole.
    pub fn done(&mut self) -> Vec<StreamEvent> {
        self.finish(FinishReason::Stop).into_iter().collect()
    }

    /// `Finished` once, however many choices or `[DONE]`s say so.
    fn finish(&mut self, reason: FinishReason) -> Option<StreamEvent> {
        (!std::mem::replace(&mut self.finished, true)).then_some(StreamEvent::Finished { reason })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::Message;

    // covers: AI-21
    // suite: unit
    #[test]
    fn request_body_streams_with_usage_and_puts_the_system_prompt_first() {
        let request = ModelRequest::new(
            "gemini-2.5-flash",
            vec![
                Message::user("hi"),
                Message::assistant("hello"),
                Message::user("why?"),
            ],
        )
        .with_system("Be brief.")
        .with_max_output_tokens(16);
        assert_eq!(
            serde_json::to_value(request_body(&request)).unwrap(),
            json!({
                "model": "gemini-2.5-flash",
                "messages": [
                    {"role": "system", "content": "Be brief."},
                    {"role": "user", "content": "hi"},
                    {"role": "assistant", "content": "hello"},
                    {"role": "user", "content": "why?"},
                ],
                "stream": true,
                "stream_options": {"include_usage": true},
                "max_tokens": 16,
            })
        );
        // As the HTTP body is written: 0.7 stays 0.7.
        let warm = ModelRequest::new("m", vec![]).with_temperature(0.7);
        let text = serde_json::to_string(&request_body(&warm)).unwrap();
        assert!(
            text.contains(r#""temperature":0.7,"#) || text.ends_with(r#""temperature":0.7}"#),
            "{text}"
        );
        assert!(text.contains(r#""messages":[]"#), "{text}");
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn chunks_from_ollama_and_gemini_become_events() {
        // Ollama: role first, text, finish, then usage with no choices.
        let mut ollama = Decoder::new("qwen2.5:0.5b");
        let mut events = Vec::new();
        for chunk in [
            r#"{"model":"qwen2.5:0.5b","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}"#,
            r#"{"model":"qwen2.5:0.5b","choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}"#,
            r#"{"model":"qwen2.5:0.5b","choices":[{"index":0,"delta":{"content":"!"},"finish_reason":"stop"}]}"#,
            r#"{"model":"qwen2.5:0.5b","choices":[],"usage":{"prompt_tokens":30,"completion_tokens":2,"total_tokens":32}}"#,
        ] {
            events.extend(ollama.chunk(chunk).unwrap());
        }
        events.extend(ollama.done());
        assert_eq!(
            events,
            vec![
                StreamEvent::Started {
                    model: "qwen2.5:0.5b".into()
                },
                StreamEvent::TextDelta("Hi".into()),
                StreamEvent::TextDelta("!".into()),
                StreamEvent::Finished {
                    reason: FinishReason::Stop
                },
                StreamEvent::Usage(Usage::new(30, 2)),
            ]
        );

        // Gemini: no model field, usage beside the finish, cached tokens.
        let mut gemini = Decoder::new("gemini-2.5-flash");
        let events = gemini
            .chunk(r#"{"choices":[{"delta":{"content":"Yes"},"finish_reason":"length"}],"usage":{"prompt_tokens":5,"completion_tokens":1,"prompt_tokens_details":{"cached_tokens":4}}}"#)
            .unwrap();
        assert_eq!(
            events,
            vec![
                StreamEvent::Started {
                    model: "gemini-2.5-flash".into()
                },
                StreamEvent::TextDelta("Yes".into()),
                StreamEvent::Finished {
                    reason: FinishReason::Length
                },
                StreamEvent::Usage(Usage::new(5, 1).with_cached(4)),
            ]
        );
        assert_eq!(gemini.done(), vec![], "finished only once");
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn null_fields_empty_names_and_llama_cpp_timings_are_accepted() {
        let mut decoder = Decoder::new("local-model");
        let mut events = Vec::new();
        for chunk in [
            // llama.cpp without --alias may send an empty model name.
            r#"{"model":"","choices":[{"delta":{"content":"a"},"finish_reason":""}]}"#,
            r#"{"choices":[{"delta":null,"finish_reason":null}]}"#,
            r#"{"choices":null,"usage":{"prompt_tokens":null,"completion_tokens":3}}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}],"timings":{"predicted_ms":12.5}}"#,
        ] {
            events.extend(decoder.chunk(chunk).unwrap());
        }
        assert_eq!(
            events,
            vec![
                StreamEvent::Started {
                    model: "local-model".into()
                },
                StreamEvent::TextDelta("a".into()),
                StreamEvent::Usage(Usage::new(0, 3)),
                StreamEvent::Finished {
                    reason: FinishReason::Stop
                },
            ]
        );
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn done_without_a_finish_reason_is_a_stop_and_bad_chunks_are_errors() {
        let mut decoder = Decoder::new("m");
        decoder
            .chunk(r#"{"choices":[{"delta":{"content":"a"}}]}"#)
            .unwrap();
        assert_eq!(
            decoder.done(),
            vec![StreamEvent::Finished {
                reason: FinishReason::Stop
            }]
        );

        let mut decoder = Decoder::new("m");
        assert!(matches!(
            decoder.chunk("{not json"),
            Err(ChunkError::Malformed(_))
        ));
        assert!(matches!(
            decoder.chunk(r#"{"choices":"x"}"#),
            Err(ChunkError::Malformed(_))
        ));
        assert_eq!(
            decoder.chunk(r#"{"error":{"message":"model is overloaded","code":503}}"#),
            Err(ChunkError::Provider("model is overloaded".into()))
        );
        assert_eq!(
            decoder.chunk(r#"{"error":"boom"}"#),
            Err(ChunkError::Provider("\"boom\"".into()))
        );
    }
}
