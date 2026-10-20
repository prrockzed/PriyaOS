//! The `openai_compatible` adapter: one adapter for every server that
//! speaks the Chat Completions API (Gemini's OpenAI endpoint, xAI, Ollama,
//! LM Studio, the llama.cpp server), configured by its base URL.

pub mod errors;
pub mod sse;
pub mod wire;
