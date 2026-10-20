//! [`FakeModelProvider`]: a model provider that plays scripted replies
//! (text, pauses, usage, errors), so model callers are tested without a
//! network or a model.
//!
//! ```ignore
//! let fake = FakeModelProvider::new("fake")
//!     .reply(Reply::answer("fake-1", &["Hello", " there"]))
//!     .reply(Reply::Refuse(busy));
//! ```

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use priya_ai::provider::BoxFuture;
use priya_ai::{
    FinishReason, Health, ModelProvider, ModelRequest, ModelStream, ProviderError,
    ProviderErrorKind, StreamEvent, Usage,
};
use tokio::sync::mpsc;

use crate::lock;

/// One step of a scripted stream.
#[derive(Debug, Clone)]
pub enum Step {
    Event(StreamEvent),
    /// Nothing happens for this long.
    Pause(Duration),
    /// The stream fails here; nothing after it is sent.
    Fail(ProviderError),
    /// Nothing more ever comes, until the stream is dropped.
    Hang,
}

/// What the fake does with one request.
#[derive(Debug, Clone)]
pub enum Reply {
    /// Fails before streaming, as a refused key or a rate limit does.
    Refuse(ProviderError),
    Stream(Vec<Step>),
}

impl Reply {
    /// A whole answer: started, the chunks, finished, then usage (one
    /// token per chunk out, ten in), the order OpenAI-style APIs use.
    pub fn answer(model: &str, chunks: &[&str]) -> Self {
        let mut steps = vec![Step::Event(StreamEvent::Started {
            model: model.to_owned(),
        })];
        steps.extend(
            chunks
                .iter()
                .map(|chunk| Step::Event(StreamEvent::TextDelta((*chunk).to_owned()))),
        );
        steps.push(Step::Event(StreamEvent::Finished {
            reason: FinishReason::Stop,
        }));
        steps.push(Step::Event(StreamEvent::Usage(Usage::new(
            10,
            chunks.len() as u64,
        ))));
        Self::Stream(steps)
    }
}

/// A scripted [`ModelProvider`]. Replies are used in order; a request with
/// none left fails with a `bad_request` that says so.
pub struct FakeModelProvider {
    id: String,
    replies: Mutex<VecDeque<Reply>>,
    requests: Mutex<Vec<ModelRequest>>,
    health: Mutex<Health>,
    /// Streams whose worker is still running, which a dropped stream must
    /// bring back to zero.
    open: Arc<AtomicUsize>,
}

impl FakeModelProvider {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            replies: Mutex::default(),
            requests: Mutex::default(),
            health: Mutex::new(Health::Ok),
            open: Arc::default(),
        }
    }

    /// Adds the next reply.
    #[must_use]
    pub fn reply(self, reply: Reply) -> Self {
        self.push(reply);
        self
    }

    /// Adds a reply to a fake already in use.
    pub fn push(&self, reply: Reply) {
        lock(&self.replies).push_back(reply);
    }

    /// What `health` reports from now on.
    pub fn set_health(&self, health: Health) {
        *lock(&self.health) = health;
    }

    /// Every request so far, oldest first.
    pub fn requests(&self) -> Vec<ModelRequest> {
        lock(&self.requests).clone()
    }

    /// How many streams are still running.
    pub fn open_streams(&self) -> usize {
        self.open.load(Ordering::SeqCst)
    }

    fn start(&self, request: ModelRequest) -> Result<ModelStream, ProviderError> {
        lock(&self.requests).push(request);
        let reply = lock(&self.replies).pop_front().unwrap_or_else(|| {
            Reply::Refuse(ProviderError::new(
                ProviderErrorKind::BadRequest,
                "The fake model has no reply left.",
            ))
        });
        let steps = match reply {
            Reply::Refuse(error) => return Err(error),
            Reply::Stream(steps) => steps,
        };
        let (tx, rx) = mpsc::channel(8);
        let open = Open::count(&self.open);
        let worker = tokio::spawn(async move {
            let _open = open;
            for step in steps {
                let item = match step {
                    Step::Event(event) => Ok(event),
                    Step::Pause(pause) => {
                        tokio::time::sleep(pause).await;
                        continue;
                    }
                    Step::Fail(error) => Err(error),
                    Step::Hang => std::future::pending().await,
                };
                let last = item.is_err();
                if tx.send(item).await.is_err() || last {
                    return;
                }
            }
        });
        Ok(ModelStream::from_worker(rx, worker))
    }
}

impl ModelProvider for FakeModelProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn complete(&self, request: ModelRequest) -> BoxFuture<'_, Result<ModelStream, ProviderError>> {
        // Nothing happens until the future runs, as with a real provider.
        Box::pin(async move { self.start(request) })
    }

    fn health(&self) -> BoxFuture<'_, Health> {
        let health = lock(&self.health).clone();
        Box::pin(async move { health })
    }
}

/// Counts a running stream until dropped, which happens when its worker
/// ends or is stopped. A short answer's worker may end before anyone reads
/// it (the channel holds 8 items), so only long answers test cancelling.
struct Open(Arc<AtomicUsize>);

impl Open {
    fn count(open: &Arc<AtomicUsize>) -> Self {
        open.fetch_add(1, Ordering::SeqCst);
        Self(open.clone())
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
