//! Timeouts and the single early retry (principle P9) around any provider,
//! so every adapter and the fake behave the same.
//!
//! - **First token:** the request must produce text (or finish) within
//!   `first_token`; local models may still be loading, so it is long.
//! - **Idle:** after that, each event must come within `idle`.
//! - **Total:** the whole answer must be done within `total`.
//! - **Retry:** one more try, after a short jittered wait, when the provider
//!   is overloaded or unreachable before any text came. Never once text has
//!   streamed: the user has seen it, and a second answer would differ. A
//!   stream that ends before any text counts as unreachable. Both tries
//!   share the first-token deadline, so the user never waits longer than
//!   it in all; anything the failed try sent (such as usage) is dropped.
//!
//! The connect timeout belongs to the HTTP adapter, which owns the socket.
//! It must report a failed connect as `network`, not `timeout`, so that it
//! gets the retry.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::mpsc;
use tokio::time::{Instant, timeout_at};
use tokio_stream::StreamExt;

use crate::error::{ProviderError, ProviderErrorKind};
use crate::model::{ModelRequest, StreamEvent};
use crate::provider::{BoxFuture, Health, ModelProvider, ModelStream};

/// How long a call may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    pub first_token: Duration,
    pub idle: Duration,
    pub total: Duration,
}

impl Default for Timeouts {
    /// The Phase 0 values from the phase file.
    fn default() -> Self {
        Self {
            first_token: Duration::from_secs(60),
            idle: Duration::from_secs(30),
            total: Duration::from_secs(5 * 60),
        }
    }
}

/// The base wait before the retry; jitter adds up to as much again.
const RETRY_WAIT: Duration = Duration::from_millis(250);
/// A provider that asks us to wait longer than this isn't retried at once:
/// the error, with its `retry_after`, goes to the user instead.
const LONGEST_RETRY_WAIT: Duration = Duration::from_secs(5);

/// A provider with timeouts and the early retry.
pub struct Reliable {
    inner: Arc<dyn ModelProvider>,
    timeouts: Timeouts,
}

impl Reliable {
    pub fn new(inner: Arc<dyn ModelProvider>, timeouts: Timeouts) -> Self {
        Self { inner, timeouts }
    }

    async fn answer(&self, request: ModelRequest) -> Result<ModelStream, ProviderError> {
        let start = Instant::now();
        let total = start + self.timeouts.total;
        let first_token = (start + self.timeouts.first_token).min(total);
        let mut retried = false;
        loop {
            match self.until_first_token(&request, first_token).await {
                Ok((seen, rest)) => return Ok(self.guard(seen, rest, total)),
                Err(e) if !retried => {
                    let Some(wait) = retry_wait(&e) else {
                        return Err(e);
                    };
                    if Instant::now() + wait >= first_token {
                        return Err(e);
                    }
                    retried = true;
                    tokio::time::sleep(wait).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Calls the provider and reads up to the first text (or the end), so
    /// a failure before any text is still an `Err` that may be retried.
    async fn until_first_token(
        &self,
        request: &ModelRequest,
        deadline: Instant,
    ) -> Result<(Vec<StreamEvent>, ModelStream), ProviderError> {
        let attempt = async {
            let mut stream = self.inner.complete(request.clone()).await?;
            let mut seen = Vec::new();
            while let Some(event) = stream.next().await {
                let event = event?;
                let first = matches!(
                    event,
                    StreamEvent::TextDelta(_) | StreamEvent::Finished { .. }
                );
                seen.push(event);
                if first {
                    return Ok((seen, stream));
                }
            }
            // ModelStream ends with an error rather than early, so this
            // only gives the loop an end.
            Err(ProviderError::new(
                ProviderErrorKind::Network,
                "The answer stopped before it was finished.",
            ))
        };
        timeout_at(deadline, attempt)
            .await
            .unwrap_or_else(|_| Err(timeout("The model took too long to start answering.")))
    }

    /// The rest of the answer, with the idle and total timeouts.
    fn guard(&self, seen: Vec<StreamEvent>, mut rest: ModelStream, total: Instant) -> ModelStream {
        let idle = self.timeouts.idle;
        let (tx, rx) = mpsc::channel(32);
        let worker = tokio::spawn(async move {
            for event in seen {
                if tx.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            loop {
                let deadline = (Instant::now() + idle).min(total);
                let item = match timeout_at(deadline, rest.next()).await {
                    Ok(Some(item)) => item,
                    Ok(None) => return,
                    Err(_) if deadline == total => {
                        Err(timeout("The answer took too long, so it was stopped."))
                    }
                    Err(_) => Err(timeout("The model stopped sending its answer.")),
                };
                let last = item.is_err();
                if tx.send(item).await.is_err() || last {
                    return;
                }
            }
        });
        ModelStream::from_worker(rx, worker)
    }
}

impl ModelProvider for Reliable {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn complete(&self, request: ModelRequest) -> BoxFuture<'_, Result<ModelStream, ProviderError>> {
        Box::pin(self.answer(request))
    }

    fn health(&self) -> BoxFuture<'_, Health> {
        self.inner.health()
    }
}

/// How long to wait before the retry, if this error is worth one.
fn retry_wait(error: &ProviderError) -> Option<Duration> {
    let worth_it = matches!(
        error.kind,
        ProviderErrorKind::Overloaded | ProviderErrorKind::Network
    );
    let asked = error.retry_after.unwrap_or(RETRY_WAIT);
    (worth_it && asked <= LONGEST_RETRY_WAIT).then(|| asked + jitter())
}

/// Up to [`RETRY_WAIT`], so clients that failed together don't retry
/// together. Where in the current second we are is random enough for that
/// (macOS counts only microseconds, so the whole fraction is used).
fn jitter() -> Duration {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    RETRY_WAIT.mul_f64(f64::from(nanos) / 1e9)
}

fn timeout(message: &str) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Timeout, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: AI-21
    // suite: unit
    #[test]
    fn only_busy_or_unreachable_providers_get_a_short_retry() {
        let error = |kind| ProviderError::new(kind, "x");
        for kind in [ProviderErrorKind::Overloaded, ProviderErrorKind::Network] {
            let wait = retry_wait(&error(kind)).unwrap();
            assert!((RETRY_WAIT..=RETRY_WAIT * 2).contains(&wait), "{wait:?}");
        }
        for kind in [
            ProviderErrorKind::Auth,
            ProviderErrorKind::BadRequest,
            ProviderErrorKind::RateLimited,
            ProviderErrorKind::Timeout,
            ProviderErrorKind::Server,
        ] {
            assert_eq!(retry_wait(&error(kind)), None, "{kind:?}");
        }
        // Asked to wait long: the user hears about it instead.
        let long =
            error(ProviderErrorKind::Overloaded).with_retry_after(Some(Duration::from_secs(30)));
        assert_eq!(retry_wait(&long), None);
        let short =
            error(ProviderErrorKind::Overloaded).with_retry_after(Some(Duration::from_secs(1)));
        assert!(retry_wait(&short).unwrap() >= Duration::from_secs(1));
        // Right at the limit: always retried, whatever the jitter.
        let edge = error(ProviderErrorKind::Network).with_retry_after(Some(LONGEST_RETRY_WAIT));
        assert!(retry_wait(&edge).is_some());
        assert_eq!(Timeouts::default().first_token, Duration::from_secs(60));
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn jitter_varies() {
        let waits: std::collections::BTreeSet<Duration> = (0..50)
            .map(|_| {
                std::thread::sleep(Duration::from_micros(137));
                jitter()
            })
            .collect();
        assert!(waits.len() > 1, "{waits:?}");
        assert!(waits.iter().all(|w| *w < RETRY_WAIT));
    }
}
