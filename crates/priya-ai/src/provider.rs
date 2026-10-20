//! The `ModelProvider` port (04-priya-ai §2), as far as Phase 0 needs it:
//! `list_models`, `capabilities` and `price` come in 1.1.
//!
//! The trait returns boxed futures rather than using `async fn`, so the
//! gateway can hold providers as `Arc<dyn ModelProvider>` and pick one at
//! run time from settings.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::{ProviderError, ProviderErrorKind};
use crate::model::{ModelRequest, StreamEvent};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Whether a provider can be used, found without spending tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    Ok,
    /// It can't be used now; the error says why (no key, unreachable…).
    Down(ProviderError),
}

/// A source of model answers: a cloud API, a local server or a fake.
pub trait ModelProvider: Send + Sync {
    /// The provider's name in settings, such as `gemini` or `local`.
    fn id(&self) -> &str;

    /// Starts a streamed answer. Errors that happen before the first event
    /// (refused key, rate limit, unreachable) come back here; later ones
    /// arrive in the stream.
    fn complete(&self, request: ModelRequest) -> BoxFuture<'_, Result<ModelStream, ProviderError>>;

    fn health(&self) -> BoxFuture<'_, Health>;
}

/// The events of one answer. Dropping it stops the work behind it, which
/// for an HTTP adapter closes the connection, so a cancelled answer stops
/// costing tokens at once.
///
/// It ends after an error, and an inner stream that ends before
/// `Finished` (a worker that panicked, a connection that closed early)
/// ends with an error instead, so a cut-short answer is never taken for a
/// whole one.
pub struct ModelStream {
    events: Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>,
    state: State,
    _worker: Option<AbortOnDrop>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Open,
    /// `Finished` came; only usage may follow.
    Finished,
    Done,
}

impl ModelStream {
    /// A stream fed by `worker` through `events`; the worker is stopped
    /// when the stream is dropped.
    pub fn from_worker(
        events: mpsc::Receiver<Result<StreamEvent, ProviderError>>,
        worker: JoinHandle<()>,
    ) -> Self {
        Self {
            events: Box::pin(ReceiverStream::new(events)),
            state: State::Open,
            _worker: Some(AbortOnDrop(worker)),
        }
    }

    /// Wraps any stream, such as another `ModelStream` with timeouts added.
    pub fn new(
        events: impl Stream<Item = Result<StreamEvent, ProviderError>> + Send + 'static,
    ) -> Self {
        Self {
            events: Box::pin(events),
            state: State::Open,
            _worker: None,
        }
    }
}

impl Stream for ModelStream {
    type Item = Result<StreamEvent, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.state == State::Done {
            return Poll::Ready(None);
        }
        let item = std::task::ready!(self.events.as_mut().poll_next(cx));
        let item = match item {
            Some(Ok(StreamEvent::Finished { reason })) => {
                self.state = State::Finished;
                Some(Ok(StreamEvent::Finished { reason }))
            }
            Some(Err(e)) => {
                self.state = State::Done;
                Some(Err(e))
            }
            None if self.state == State::Open => {
                self.state = State::Done;
                Some(Err(ProviderError::new(
                    ProviderErrorKind::Network,
                    "The answer stopped before it was finished.",
                )
                .with_detail("the stream ended without a finish reason")))
            }
            other => other,
        };
        Poll::Ready(item)
    }
}

impl fmt::Debug for ModelStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelStream")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio_stream::StreamExt;

    use super::*;
    use crate::model::{FinishReason, Usage};

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn dropping_a_stream_stops_its_worker() {
        let (tx, rx) = mpsc::channel(4);
        let (done_tx, mut done_rx) = mpsc::channel::<()>(1);
        let worker = tokio::spawn(async move {
            // Held until the task ends, so its end is visible.
            let _done = done_tx;
            tx.send(Ok(StreamEvent::TextDelta("hi".into())))
                .await
                .unwrap();
            std::future::pending::<()>().await;
        });
        let mut stream = ModelStream::from_worker(rx, worker);
        assert_eq!(
            stream.next().await.unwrap().unwrap(),
            StreamEvent::TextDelta("hi".into())
        );
        drop(stream);
        let ended = tokio::time::timeout(Duration::from_secs(1), done_rx.recv()).await;
        assert_eq!(ended, Ok(None), "the worker was stopped");
    }

    fn events(
        items: Vec<Result<StreamEvent, ProviderError>>,
    ) -> Vec<Result<StreamEvent, ProviderError>> {
        let stream = ModelStream::new(tokio_stream::iter(items));
        assert!(format!("{stream:?}").starts_with("ModelStream"));
        collect_blocking(stream)
    }

    fn collect_blocking(stream: ModelStream) -> Vec<Result<StreamEvent, ProviderError>> {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(stream.collect())
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn a_stream_ends_after_an_error_and_a_cut_short_one_is_an_error() {
        let delta = |t: &str| Ok(StreamEvent::TextDelta(t.into()));
        let finished = Ok(StreamEvent::Finished {
            reason: FinishReason::Stop,
        });
        let usage = Ok(StreamEvent::Usage(Usage::new(1, 2)));

        // Whole: usage may come after the finish reason.
        let whole = events(vec![delta("a"), finished.clone(), usage.clone()]);
        assert_eq!(whole, vec![delta("a"), finished, usage]);

        // Cut short: an error takes the place of the missing end.
        let cut = events(vec![delta("a")]);
        assert_eq!(cut.len(), 2);
        let error = cut[1].clone().unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::Network);
        assert_eq!(
            error.user_message,
            "The answer stopped before it was finished."
        );

        // Nothing after an error.
        let busy = ProviderError::new(ProviderErrorKind::Overloaded, "busy");
        let failed = events(vec![delta("a"), Err(busy.clone()), delta("b")]);
        assert_eq!(failed, vec![delta("a"), Err(busy)]);
    }
}
