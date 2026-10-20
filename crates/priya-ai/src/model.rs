//! What a model is asked and what it streams back (04-priya-ai §2). These
//! are the minimal Phase 0 types; 1.1 adds tools, images, cache hints and
//! effort without changing what is here.

/// Who said a message in a conversation. Tool results join in 1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Role {
    User,
    Assistant,
}

/// One turn of a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            text: text.into(),
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            text: text.into(),
        }
    }
}

/// One request to a model. Built with [`ModelRequest::new`] and the
/// `with_*` methods, so 1.1 can add tools, images and cache hints without
/// breaking callers.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ModelRequest {
    /// The model's name at the provider, such as `gemini-2.5-flash`. The
    /// phase file leaves it out; a provider serves many models, so the
    /// request has to say which.
    pub model: String,
    /// Instructions that come before the conversation.
    pub system: Option<String>,
    /// The conversation so far, oldest first; the last is the question.
    pub messages: Vec<Message>,
    pub max_output_tokens: u32,
    /// The provider's default when absent.
    pub temperature: Option<f32>,
}

impl ModelRequest {
    /// The default answer length until settings say otherwise.
    pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 2048;

    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            system: None,
            messages,
            max_output_tokens: Self::DEFAULT_MAX_OUTPUT_TOKENS,
            temperature: None,
        }
    }

    #[must_use]
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    #[must_use]
    pub fn with_max_output_tokens(mut self, tokens: u32) -> Self {
        self.max_output_tokens = tokens;
        self
    }

    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

/// Tokens a call used, as the provider reports them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    /// Input tokens served from the provider's cache; `None` when the
    /// provider doesn't say, which isn't the same as none cached.
    pub cached: Option<u64>,
}

impl Usage {
    pub fn new(input: u64, output: u64) -> Self {
        Self {
            input,
            output,
            cached: None,
        }
    }

    #[must_use]
    pub fn with_cached(mut self, cached: u64) -> Self {
        self.cached = Some(cached);
        self
    }
}

/// Why a model stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FinishReason {
    /// It finished its answer.
    Stop,
    /// It reached `max_output_tokens`.
    Length,
    /// The provider's safety filter stopped it.
    ContentFilter,
    /// Anything else, as the provider said it.
    Other(String),
}

impl FinishReason {
    /// The reason from the name a provider uses for it.
    pub fn from_provider(name: &str) -> Self {
        match name {
            "stop" | "end_turn" | "stop_sequence" => Self::Stop,
            "length" | "max_tokens" => Self::Length,
            "content_filter" | "safety" => Self::ContentFilter,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// What a model stream yields: `Started` once, then text, then `Finished`
/// once and `Usage` if the provider reports it, in the provider's order
/// (OpenAI-style APIs send usage after the finish reason). A provider that
/// counts as it goes sends `Usage` more than once; the last one wins. An `Err` item
/// is always the last one; a stream that ends without `Finished` or an
/// error is turned into an error by `ModelStream`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StreamEvent {
    /// The provider accepted the request; `model` is the exact model that
    /// answers, which may be more specific than the one asked for.
    Started {
        model: String,
    },
    TextDelta(String),
    Usage(Usage),
    Finished {
        reason: FinishReason,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: AI-21
    // suite: unit
    #[test]
    fn finish_reasons_read_every_providers_names() {
        assert_eq!(FinishReason::from_provider("stop"), FinishReason::Stop);
        assert_eq!(FinishReason::from_provider("end_turn"), FinishReason::Stop);
        assert_eq!(
            FinishReason::from_provider("stop_sequence"),
            FinishReason::Stop
        );
        assert_eq!(FinishReason::from_provider("length"), FinishReason::Length);
        assert_eq!(
            FinishReason::from_provider("max_tokens"),
            FinishReason::Length
        );
        assert_eq!(
            FinishReason::from_provider("content_filter"),
            FinishReason::ContentFilter
        );
        assert_eq!(
            FinishReason::from_provider("tool_calls"),
            FinishReason::Other("tool_calls".into())
        );
        assert_eq!(Message::user("hi").role, Role::User);
        assert_eq!(Message::assistant("hello").text, "hello");
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn requests_and_usage_are_built_step_by_step() {
        let request = ModelRequest::new("qwen2.5:0.5b", vec![Message::user("hi")])
            .with_system("Be brief.")
            .with_max_output_tokens(16)
            .with_temperature(0.2);
        assert_eq!(request.model, "qwen2.5:0.5b");
        assert_eq!(request.system.as_deref(), Some("Be brief."));
        assert_eq!(request.max_output_tokens, 16);
        assert_eq!(request.temperature, Some(0.2));
        let plain = ModelRequest::new("m", Vec::new());
        assert_eq!(
            plain.max_output_tokens,
            ModelRequest::DEFAULT_MAX_OUTPUT_TOKENS
        );
        assert_eq!(plain.temperature, None);

        assert_eq!(Usage::new(3, 4).cached, None);
        assert_eq!(Usage::new(3, 4).with_cached(0).cached, Some(0));
    }
}
