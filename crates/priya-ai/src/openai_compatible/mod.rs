//! The `openai_compatible` adapter: one adapter for every server that
//! speaks the Chat Completions API (Gemini's OpenAI endpoint, xAI, Ollama,
//! LM Studio, the llama.cpp server), configured by its base URL.
//!
//! Errors before the answer starts come back from `complete`, mapped from
//! the HTTP status to a plain sentence naming the provider; errors while
//! streaming end the stream. Timeouts other than the connect timeout and
//! the retry are `Reliable`'s, which the gateway wraps around this.

pub mod errors;
pub mod sse;
pub mod wire;

use std::sync::Arc;
use std::time::Duration;

use priya_core::secret::Secret;
use reqwest::header::RETRY_AFTER;
use reqwest::redirect::Policy;
use reqwest::{Client, RequestBuilder, Response};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

use self::errors::{
    Provider, Purpose, body_detail, capital, chain, redact, retry_after, status_error,
};
use self::sse::{SseEvent, SseParser};
use self::wire::{ChunkError, Decoder};
use crate::error::{ProviderError, ProviderErrorKind};
use crate::model::{ModelRequest, StreamEvent};
use crate::provider::{BoxFuture, Health, ModelProvider, ModelStream};

/// How long a connection may take to open (P9).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the health check may take; it must never hold up `doctor`.
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);
/// The most of an error body read: enough for any real error message, and
/// a server can't make the daemon read megabytes for one.
const ERROR_BODY_LIMIT: usize = 16 * 1024;

/// Where a provider is and how to talk to it.
#[derive(Debug, Clone)]
pub struct Endpoint {
    /// The provider's name in settings, such as `gemini`.
    pub id: String,
    /// How messages name it mid-sentence: `Gemini`, `the local model`.
    pub label: String,
    /// Such as `http://127.0.0.1:11434/v1`.
    pub base_url: String,
    /// Sent as a bearer token; local servers have none.
    pub key: Option<Arc<Secret<String>>>,
}

/// The HTTP client the adapters share (one connection pool).
pub fn http_client() -> Result<Client, ProviderError> {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        // A redirect would turn the POST into a GET without its body; an
        // address that moved is reported instead.
        .redirect(Policy::none())
        .user_agent(concat!("PriyaOS/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::Network,
                "PriyaOS couldn't set up its connections to model providers.",
            )
            .with_detail(e.to_string())
        })
}

pub struct OpenAiCompatible {
    endpoint: Endpoint,
    http: Client,
}

impl OpenAiCompatible {
    pub fn new(endpoint: Endpoint, http: Client) -> Self {
        Self { endpoint, http }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.endpoint.base_url.trim_end_matches('/'))
    }

    fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.endpoint.key {
            Some(key) => request.bearer_auth(key.expose()),
            None => request,
        }
    }

    async fn start(&self, request: ModelRequest) -> Result<ModelStream, ProviderError> {
        let response = self
            .authorized(self.http.post(self.url("chat/completions")))
            .json(&wire::request_body(&request))
            .send()
            .await
            .map_err(|e| self.unreachable(&e))?;
        let purpose = Purpose::Answer {
            model: &request.model,
        };
        let response = self.check(response, purpose).await?;
        let (tx, rx) = mpsc::channel(32);
        let messages = Messages {
            label: self.endpoint.label.clone(),
            key: self.endpoint.key.clone(),
        };
        let worker = tokio::spawn(read_stream(
            response,
            Decoder::new(&request.model),
            messages,
            tx,
        ));
        Ok(ModelStream::from_worker(rx, worker))
    }

    /// The response if it succeeded, else its status as an error.
    async fn check(
        &self,
        mut response: Response,
        purpose: Purpose<'_>,
    ) -> Result<Response, ProviderError> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let header = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut body = Vec::new();
        while body.len() < ERROR_BODY_LIMIT {
            match response.chunk().await {
                Ok(Some(chunk)) => body.extend_from_slice(&chunk),
                _ => break,
            }
        }
        let body = String::from_utf8_lossy(&body);
        let provider = Provider {
            id: &self.endpoint.id,
            label: &self.endpoint.label,
            base_url: &self.endpoint.base_url,
            has_key: self.endpoint.key.is_some(),
        };
        let wait = retry_after(header.as_deref(), &body, time::OffsetDateTime::now_utc());
        let key = self.endpoint.key.as_ref().map(|k| k.expose().as_str());
        Err(
            status_error(&provider, status.as_u16(), purpose, &body, wait)
                .with_detail(body_detail(&status.to_string(), &body, key)),
        )
    }

    /// A request that got no response. Connect timeouts count as network
    /// errors, so `Reliable` retries them.
    fn unreachable(&self, error: &reqwest::Error) -> ProviderError {
        ProviderError::new(
            ProviderErrorKind::Network,
            format!("Couldn't reach {}.", self.endpoint.label),
        )
        .with_detail(self.redacted(&chain(error)))
    }

    fn redacted(&self, text: &str) -> String {
        redact(
            text,
            self.endpoint.key.as_ref().map(|k| k.expose().as_str()),
        )
    }

    async fn check_health(&self) -> Health {
        let sent = self
            .authorized(self.http.get(self.url("models")))
            .timeout(HEALTH_TIMEOUT)
            .send()
            .await;
        let response = match sent {
            Ok(response) => response,
            Err(e) => return Health::Down(self.unreachable(&e)),
        };
        match self.check(response, Purpose::Health).await {
            Ok(_) => Health::Ok,
            Err(e) => Health::Down(e),
        }
    }
}

impl ModelProvider for OpenAiCompatible {
    fn id(&self) -> &str {
        &self.endpoint.id
    }

    fn complete(&self, request: ModelRequest) -> BoxFuture<'_, Result<ModelStream, ProviderError>> {
        Box::pin(self.start(request))
    }

    fn health(&self) -> BoxFuture<'_, Health> {
        Box::pin(self.check_health())
    }
}

/// The errors a stream can end with, naming the provider.
struct Messages {
    label: String,
    key: Option<Arc<Secret<String>>>,
}

impl Messages {
    fn error(&self, kind: ProviderErrorKind, message: String, detail: &str) -> ProviderError {
        let key = self.key.as_ref().map(|k| k.expose().as_str());
        ProviderError::new(kind, message).with_detail(redact(detail, key))
    }

    fn broke(&self, error: &reqwest::Error) -> ProviderError {
        let message = format!("The connection to {} broke.", self.label);
        self.error(ProviderErrorKind::Network, message, &chain(error))
    }

    fn unreadable(&self, detail: impl ToString) -> ProviderError {
        let message = format!(
            "{} sent an answer PriyaOS couldn't read.",
            capital(&self.label)
        );
        self.error(ProviderErrorKind::Server, message, &detail.to_string())
    }

    fn failed(&self, detail: impl ToString) -> ProviderError {
        let message = format!("{} had a problem while answering.", capital(&self.label));
        self.error(ProviderErrorKind::Server, message, &detail.to_string())
    }
}

/// Reads the body, turning its events into stream items until `[DONE]`,
/// an error, or the end.
async fn read_stream(
    response: Response,
    mut decoder: Decoder,
    messages: Messages,
    tx: mpsc::Sender<Result<StreamEvent, ProviderError>>,
) {
    let mut body = response.bytes_stream();
    let mut parser = SseParser::default();
    loop {
        let (events, end) = match body.next().await {
            Some(Ok(bytes)) => (parser.push(&bytes), false),
            Some(Err(e)) => {
                let _ = tx.send(Err(messages.broke(&e))).await;
                return;
            }
            // The connection closed; a stream without `Finished` becomes
            // an error in `ModelStream`.
            None => (parser.finish().map(|last| last.into_iter().collect()), true),
        };
        let events = match events {
            Ok(events) => events,
            Err(e) => {
                let _ = tx.send(Err(messages.unreadable(e))).await;
                return;
            }
        };
        for event in events {
            let (items, last) = match event {
                SseEvent::Data(data) => match decoder.chunk(&data) {
                    Ok(items) => (items.into_iter().map(Ok).collect(), false),
                    Err(ChunkError::Malformed(e)) => (vec![Err(messages.unreadable(e))], true),
                    Err(ChunkError::Provider(e)) => (vec![Err(messages.failed(e))], true),
                },
                SseEvent::Done => (decoder.done().into_iter().map(Ok).collect(), true),
                SseEvent::Error(e) => (vec![Err(messages.failed(e))], true),
            };
            for item in items {
                if tx.send(item).await.is_err() {
                    return;
                }
            }
            if last {
                return;
            }
        }
        if end {
            return;
        }
    }
}
