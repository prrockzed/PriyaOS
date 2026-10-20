//! [`Peer`]: one end of a JSON-RPC 2.0 conversation where both sides make
//! calls, as priyad and a module do (02-core §5.2).
//!
//! A peer reads whole messages from a channel and writes them to another,
//! so the same code runs over a process's stdio (with [`lsp`] framing) and
//! over in-process channels (a builtin module). Requests from the other
//! side go to a [`Handler`], up to [`MAX_IN_FLIGHT`] at once (more are
//! answered "busy" straight away); notifications go to it one at a time,
//! in order. Answers to our own calls go back to whoever is waiting.
//!
//! The peer never stops reading while the conversation lasts, so a handler
//! may itself call the other side and wait for the answer.
//!
//! How it ends ([`Ended`]):
//! - **Closed.** The other side stops sending, or we [`close`](Peer::close)
//!   our side. Requests in flight are still answered and queued
//!   notifications handled.
//! - **Broken.** Traffic that isn't JSON-RPC (not JSON, a batch, an answer
//!   that can't be read or isn't ours, a message over [`MAX_MESSAGE`]), a
//!   flood of notifications, or a transport failure. Work in flight is
//!   dropped.
//!
//! Either way, every call waiting for an answer, and every call made
//! afterwards, fails with [`CallError::Closed`], and the outgoing channel
//! is closed once the last answer is sent, so a writer draining it
//! finishes.
//!
//! [`lsp`]: crate::framing::lsp

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde_json::Value;
use tokio::sync::{Semaphore, mpsc, oneshot, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::error::{ApiError, ErrorKind};
use crate::framing::lsp::MAX_MESSAGE;
use crate::methods::Method;
use crate::rpc::{Id, Outcome, Request, Response, RpcError, parse_request};

/// One message as it arrives: its body, or why the stream broke.
pub type Inbound = Result<Vec<u8>, String>;

/// A boxed future, for [`Handler`].
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The most requests from the other side handled at once; one more is
/// answered "busy" without being handled.
pub const MAX_IN_FLIGHT: usize = 64;

/// The most notifications waiting to be handled; one more breaks the
/// conversation, since dropping it would lose something silently.
pub const MAX_QUEUED_NOTIFICATIONS: usize = 1024;

/// Answers the other side's requests. A notification is handled the same
/// way and its answer dropped.
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, method: String, params: Option<Value>) -> BoxFuture<Result<Value, RpcError>>;
}

impl<F, Fut> Handler for F
where
    F: Fn(String, Option<Value>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Value, RpcError>> + Send + 'static,
{
    fn handle(&self, method: String, params: Option<Value>) -> BoxFuture<Result<Value, RpcError>> {
        Box::pin(self(method, params))
    }
}

/// Why a call got no result.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CallError {
    /// The conversation is over (or ended while waiting).
    #[error("the other side closed the connection")]
    Closed,
    /// The other side answered with an error.
    #[error("{}", .0.data)]
    Rpc(RpcError),
    #[error("the message is {0} bytes, more than the {MAX_MESSAGE}-byte limit")]
    TooLarge(usize),
    /// The params couldn't be written or the result couldn't be read as
    /// the method's types.
    #[error("the message doesn't match the method's types: {0}")]
    Mismatch(String),
}

/// How a conversation ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    /// The other side stopped sending, or we closed our side.
    Closed,
    /// The protocol or the transport broke; the reason says how.
    Broken(String),
}

/// Our end of the conversation. Cheap to clone; every clone makes calls on
/// the same connection.
#[derive(Clone)]
pub struct Peer {
    shared: Arc<Shared>,
}

/// Calls waiting for answers; `None` once no answer can come.
type Pending = Option<HashMap<i64, oneshot::Sender<Outcome>>>;

struct Shared {
    /// Taken when the conversation ends, so the channel closes once the
    /// last message in flight is sent and the writer can finish.
    outbound: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    pending: Mutex<Pending>,
    next_id: AtomicI64,
    /// Set by [`Peer::close`].
    closing: watch::Sender<bool>,
}

impl Shared {
    fn pending(&self) -> MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    async fn send(&self, message: &impl serde::Serialize) -> Result<(), CallError> {
        let body = serde_json::to_vec(message).map_err(|e| CallError::Mismatch(e.to_string()))?;
        if body.len() > MAX_MESSAGE {
            return Err(CallError::TooLarge(body.len()));
        }
        let outbound = self
            .outbound
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or(CallError::Closed)?;
        outbound.send(body).await.map_err(|_| CallError::Closed)
    }

    /// Answers the request `id` with `outcome`; an answer too big to send
    /// becomes an error.
    async fn answer(&self, id: Id, outcome: Result<Value, RpcError>) {
        let response = match outcome {
            Ok(result) => Response::ok(id.clone(), result),
            Err(error) => Response::err(id.clone(), error),
        };
        if let Err(CallError::TooLarge(_)) = self.send(&response).await {
            let error = RpcError::internal("the answer is over the 16 MiB limit");
            let _ = self.send(&Response::err(id, error)).await;
        }
    }
}

impl Peer {
    /// Starts the conversation: reads `inbound`, writes `outbound`, and
    /// hands the other side's requests to the handler `make` returns. It is
    /// given this peer, so it can call the other side too. The task ends
    /// when the conversation does, saying how.
    pub fn spawn<H: Handler>(
        inbound: mpsc::Receiver<Inbound>,
        outbound: mpsc::Sender<Vec<u8>>,
        make: impl FnOnce(Peer) -> H,
    ) -> (Self, JoinHandle<Ended>) {
        let shared = Arc::new(Shared {
            outbound: Mutex::new(Some(outbound)),
            pending: Mutex::new(Some(HashMap::new())),
            next_id: AtomicI64::new(1),
            closing: watch::channel(false).0,
        });
        let peer = Self { shared };
        let handler: Arc<dyn Handler> = Arc::new(make(peer.clone()));
        let task = tokio::spawn(run(peer.shared.clone(), inbound, handler));
        (peer, task)
    }

    /// Calls method `M` and waits for its result.
    pub async fn call<M: Method>(&self, params: &M::Params) -> Result<M::Result, CallError> {
        let params =
            serde_json::to_value(params).map_err(|e| CallError::Mismatch(e.to_string()))?;
        let result = self.call_raw(M::NAME, params).await?;
        serde_json::from_value(result).map_err(|e| CallError::Mismatch(e.to_string()))
    }

    /// Calls `method` with JSON params and waits for the JSON result.
    /// Dropping the future (a timeout) forgets the call; a late answer is
    /// then ignored.
    pub async fn call_raw(&self, method: &str, params: Value) -> Result<Value, CallError> {
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.shared
            .pending()
            .as_mut()
            .ok_or(CallError::Closed)?
            .insert(id, tx);
        let _forget = Forget {
            shared: &self.shared,
            id,
        };
        self.shared
            .send(&Request::call(Id::from(id), method, params))
            .await?;
        match rx.await {
            Ok(Outcome::Result(result)) => Ok(result),
            Ok(Outcome::Error(error)) => Err(CallError::Rpc(error)),
            Err(_) => Err(CallError::Closed),
        }
    }

    /// Sends a notification, which gets no answer.
    pub async fn notify(&self, method: &str, params: Value) -> Result<(), CallError> {
        self.shared
            .send(&Request::notification(method, params))
            .await
    }

    /// Ends the conversation from our side: stops reading, answers the
    /// requests in flight, then closes the outgoing channel. A module calls
    /// it when told to shut down.
    pub fn close(&self) {
        self.shared.closing.send_replace(true);
    }
}

/// Removes a call from the waiting list however its future ends.
struct Forget<'a> {
    shared: &'a Shared,
    id: i64,
}

impl Drop for Forget<'_> {
    fn drop(&mut self) {
        if let Some(pending) = self.shared.pending().as_mut() {
            pending.remove(&self.id);
        }
    }
}

/// Stops a task when dropped, so work spawned for a request ends with it.
struct AbortOnDrop<T>(JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Runs `handler` on one message in a task of its own, so a panic becomes
/// an internal error instead of an answer that never comes.
async fn guarded(
    handler: &Arc<dyn Handler>,
    method: String,
    params: Option<Value>,
) -> Result<Value, RpcError> {
    let mut task = AbortOnDrop(tokio::spawn(handler.handle(method, params)));
    match (&mut task.0).await {
        Ok(outcome) => outcome,
        Err(e) => Err(RpcError::internal(format!("the request failed: {e}"))),
    }
}

async fn run(
    shared: Arc<Shared>,
    mut inbound: mpsc::Receiver<Inbound>,
    handler: Arc<dyn Handler>,
) -> Ended {
    let limit = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
    let mut requests = JoinSet::new();
    let (notes, mut queued) = mpsc::channel::<Request>(MAX_QUEUED_NOTIFICATIONS);
    let mut notifications = AbortOnDrop(tokio::spawn({
        let handler = handler.clone();
        async move {
            while let Some(note) = queued.recv().await {
                let _ = guarded(&handler, note.method, note.params).await;
            }
        }
    }));
    let mut closing = shared.closing.subscribe();
    let ended = loop {
        let next = tokio::select! {
            biased;
            _ = closing.wait_for(|closing| *closing) => None,
            next = inbound.recv() => next,
        };
        let body = match next {
            None => break Ended::Closed,
            Some(Err(reason)) => break Ended::Broken(reason),
            Some(Ok(body)) => body,
        };
        // Reap finished requests so the set doesn't grow.
        while requests.try_join_next().is_some() {}
        match message(&body) {
            Message::Broken(reason) => break Ended::Broken(reason),
            Message::Answer(id, outcome) => {
                let waiting = shared.pending().as_mut().and_then(|p| p.remove(&id));
                // An answer nobody waits for any more (its call timed out)
                // is dropped.
                if let Some(waiting) = waiting {
                    let _ = waiting.send(outcome);
                }
            }
            Message::Refused(id, error) => {
                let shared = shared.clone();
                requests.spawn(async move { shared.answer(id, Err(error)).await });
            }
            Message::Ignored => {}
            Message::Request(request) => match request.id.clone() {
                None => {
                    if notes.try_send(request).is_err() {
                        break Ended::Broken(format!(
                            "more than {MAX_QUEUED_NOTIFICATIONS} notifications are waiting"
                        ));
                    }
                }
                Some(id) => {
                    let shared = shared.clone();
                    let Ok(permit) = limit.clone().try_acquire_owned() else {
                        requests.spawn(async move { shared.answer(id, Err(busy())).await });
                        continue;
                    };
                    let handler = handler.clone();
                    requests.spawn(async move {
                        let outcome = guarded(&handler, request.method, request.params).await;
                        shared.answer(id, outcome).await;
                        drop(permit);
                    });
                }
            },
        }
    };
    // No answer can come any more: waiting calls fail now, and so do calls
    // made from here on, including by handlers still running.
    shared.pending().take();
    drop(notes);
    match ended {
        Ended::Closed => {
            while requests.join_next().await.is_some() {}
            let _ = (&mut notifications.0).await;
        }
        Ended::Broken(_) => requests.shutdown().await,
    }
    shared
        .outbound
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    ended
}

/// What one incoming message is.
enum Message {
    Request(Request),
    Answer(i64, Outcome),
    /// A request too malformed to handle, answered with this error.
    Refused(Id, RpcError),
    /// A malformed request without an id: nobody to tell.
    Ignored,
    Broken(String),
}

fn message(body: &[u8]) -> Message {
    if body.len() > MAX_MESSAGE {
        return Message::Broken(format!(
            "a message of {} bytes is over the {MAX_MESSAGE}-byte limit",
            body.len()
        ));
    }
    let value: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(e) => return Message::Broken(format!("a message isn't JSON: {e}")),
    };
    if value.is_array() {
        return Message::Broken("batches aren't used between priyad and modules".into());
    }
    if value.get("method").is_some() {
        return match parse_request(value) {
            Ok(request) => Message::Request(request),
            Err((Id::Null, _)) => Message::Ignored,
            Err((id, error)) => Message::Refused(id, error),
        };
    }
    let response: Response = match serde_json::from_value(value) {
        Ok(response) => response,
        Err(e) => {
            return Message::Broken(format!("a message is neither a request nor an answer: {e}"));
        }
    };
    // Our calls are numbered from 1, so anything else isn't an answer to
    // us; a null id means a message of ours couldn't be read.
    let ours = match &response.id {
        Id::Number(n) => n.as_i64().filter(|n| *n > 0),
        _ => None,
    };
    match (ours, response.id, response.outcome) {
        (Some(id), _, outcome) => Message::Answer(id, outcome),
        (None, Id::Null, Outcome::Error(error)) => Message::Broken(format!(
            "the other side couldn't read a message: {}",
            error.data.detail.unwrap_or(error.message)
        )),
        (None, id, _) => Message::Broken(format!("an answer to {id:?}, which was never asked")),
    }
}

fn busy() -> RpcError {
    RpcError::from_api(
        ApiError::new(
            ErrorKind::Unavailable,
            "Too many requests at once; try again.",
        )
        .with_detail(format!("more than {MAX_IN_FLIGHT} requests in flight")),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;
    use crate::module::protocol::{Empty, Initialize, InitializeParams, Ping, Shutdown};

    type Echo = fn(String, Option<Value>) -> BoxFuture<Result<Value, RpcError>>;

    /// Two peers wired to each other, each with its handler.
    fn pair<L: Handler, R: Handler>(
        left: impl FnOnce(Peer) -> L,
        right: impl FnOnce(Peer) -> R,
    ) -> ((Peer, JoinHandle<Ended>), (Peer, JoinHandle<Ended>)) {
        let (to_right, from_left) = mpsc::channel::<Vec<u8>>(8);
        let (to_left, from_right) = mpsc::channel::<Vec<u8>>(8);
        let (left_in, left_rx) = mpsc::channel(8);
        let (right_in, right_rx) = mpsc::channel(8);
        forward(from_right, left_in);
        forward(from_left, right_in);
        (
            Peer::spawn(left_rx, to_right, left),
            Peer::spawn(right_rx, to_left, right),
        )
    }

    fn forward(mut from: mpsc::Receiver<Vec<u8>>, to: mpsc::Sender<Inbound>) {
        tokio::spawn(async move {
            while let Some(body) = from.recv().await {
                if to.send(Ok(body)).await.is_err() {
                    break;
                }
            }
        });
    }

    async fn echo(method: String, params: Option<Value>) -> Result<Value, RpcError> {
        match method.as_str() {
            "ping" | "shutdown" => Ok(json!({})),
            "fail" => Err(RpcError::from_api(ApiError::new(
                ErrorKind::NotFound,
                "Nothing here.",
            ))),
            "slow" => {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(Value::Null)
            }
            "panic" => panic!("a handler bug"),
            "huge" => Ok(json!("x".repeat(MAX_MESSAGE))),
            _ => Ok(json!({"method": method, "params": params})),
        }
    }

    fn echoing(_: Peer) -> Echo {
        |method, params| Box::pin(echo(method, params))
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn both_sides_call_each_other_and_errors_keep_their_kind() {
        let ((left, _), (right, _)) = pair(echoing, echoing);
        let answer = left.call_raw("x.say", json!({"text": "hi"})).await.unwrap();
        assert_eq!(answer, json!({"method": "x.say", "params": {"text": "hi"}}));
        right.call::<Ping>(&Empty {}).await.unwrap();
        let Err(CallError::Rpc(error)) = right.call_raw("fail", json!({})).await else {
            panic!("an error");
        };
        assert_eq!(error.data.kind, ErrorKind::NotFound);
        assert_eq!(error.data.user_message, "Nothing here.");
        assert_eq!(
            CallError::Rpc(error).to_string(),
            "not_found: Nothing here."
        );

        // Calls overlap: a slow one doesn't hold up the next.
        let slow = tokio::spawn({
            let left = left.clone();
            async move { left.call_raw("slow", json!({})).await }
        });
        left.call::<Shutdown>(&Empty {}).await.unwrap();
        slow.abort();

        // A call given up on is forgotten; its late answer is ignored.
        let gave_up =
            tokio::time::timeout(Duration::from_millis(20), left.call_raw("slow", json!({}))).await;
        assert!(gave_up.is_err());
        assert!(left.shared.pending().as_ref().unwrap().is_empty());

        // A handler that panics answers with an internal error.
        let Err(CallError::Rpc(error)) = left.call_raw("panic", json!({})).await else {
            panic!("an error");
        };
        assert_eq!(error.data.kind, ErrorKind::Internal);
        // An answer too big to send becomes an error.
        let Err(CallError::Rpc(error)) = left.call_raw("huge", json!({})).await else {
            panic!("an error");
        };
        assert!(error.data.detail.unwrap().contains("16 MiB"));

        // Notifications get no answer, and results must fit the method.
        left.notify("x.note", json!({})).await.unwrap();
        let params = InitializeParams {
            api: "v1".into(),
            host_version: "0".into(),
        };
        let mismatch = right.call::<Initialize>(&params).await;
        assert!(matches!(mismatch, Err(CallError::Mismatch(_))));
        let too_big = left.call_raw("x", json!("x".repeat(MAX_MESSAGE))).await;
        assert!(matches!(too_big, Err(CallError::TooLarge(_))));
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn handlers_may_call_back_even_past_the_in_flight_limit() {
        // Every request the right side gets asks the left side something
        // before answering, as a module asking the host for a setting.
        let calling_back = |peer: Peer| {
            move |_: String, _: Option<Value>| {
                let peer = peer.clone();
                async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    let answer = peer.call_raw("x.inner", json!({})).await;
                    answer.map_err(|e| RpcError::internal(e.to_string()))
                }
            }
        };
        let ((left, _), _) = pair(echoing, calling_back);
        let calls: Vec<_> = (0..MAX_IN_FLIGHT + 36)
            .map(|_| {
                let left = left.clone();
                tokio::spawn(async move { left.call_raw("x.outer", json!({})).await })
            })
            .collect();
        let (mut answered, mut busy) = (0, 0);
        for call in calls {
            let outcome = tokio::time::timeout(Duration::from_secs(10), call)
                .await
                .expect("no deadlock")
                .unwrap();
            match outcome {
                Ok(_) => answered += 1,
                Err(CallError::Rpc(e)) if e.data.kind == ErrorKind::Unavailable => {
                    assert!(e.data.retryable);
                    busy += 1;
                }
                Err(e) => panic!("{e}"),
            }
        }
        assert_eq!(answered + busy, MAX_IN_FLIGHT + 36);
        assert!(answered >= MAX_IN_FLIGHT, "{answered} answered");
        assert!(busy > 0, "the limit was reached");
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn notifications_are_handled_one_at_a_time_in_order() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recording = {
            let seen = seen.clone();
            move |_: Peer| {
                move |_: String, params: Option<Value>| {
                    let seen = seen.clone();
                    async move {
                        // Later notes would overtake a slow one if they ran
                        // side by side.
                        let n = params.and_then(|p| p["n"].as_u64()).unwrap_or_default();
                        tokio::time::sleep(Duration::from_millis(20 - n)).await;
                        seen.lock().unwrap().push(n);
                        Ok(json!({}))
                    }
                }
            }
        };
        let ((left, _), _) = pair(echoing, recording);
        for n in 0..10 {
            left.notify("note", json!({"n": n})).await.unwrap();
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            while seen.lock().unwrap().len() < 10 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(*seen.lock().unwrap(), (0..10).collect::<Vec<u64>>());
    }

    /// A peer fed `bodies` as the other side's messages; what it wrote back.
    async fn fed(bodies: Vec<Inbound>) -> (Ended, Vec<Value>) {
        let (tx, rx) = mpsc::channel(bodies.len() + 1);
        let (out, mut written) = mpsc::channel(16);
        for body in bodies {
            tx.send(body).await.unwrap();
        }
        drop(tx);
        let (_peer, task) = Peer::spawn(rx, out, echoing);
        let ended = task.await.unwrap();
        let mut replies = Vec::new();
        while let Some(body) = written.recv().await {
            replies.push(serde_json::from_slice(&body).unwrap());
        }
        (ended, replies)
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn garbage_breaks_the_conversation_and_waiting_calls_learn_it() {
        let text = |text: &str| Inbound::Ok(text.as_bytes().to_vec());
        let parse_error =
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}"#;
        for (bodies, why) in [
            (vec![text("not json")], "isn't JSON"),
            (vec![text("[]")], "batches"),
            (vec![text("{\"jsonrpc\":\"2.0\",\"id\":1}")], "neither"),
            (vec![Err("framing broke".into())], "framing broke"),
            (vec![Ok(vec![b' '; MAX_MESSAGE + 1])], "over the"),
            (
                vec![text(r#"{"jsonrpc":"2.0","id":"a","result":1}"#)],
                "never asked",
            ),
            (
                vec![text(r#"{"jsonrpc":"2.0","id":0,"result":1}"#)],
                "never asked",
            ),
            (vec![text(parse_error)], "couldn't read"),
        ] {
            let (ended, _) = fed(bodies).await;
            let Ended::Broken(reason) = ended else {
                panic!("broken: {why}");
            };
            assert!(reason.contains(why), "{reason}");
        }

        // A malformed request is answered when it has an id, and the
        // conversation goes on; one without an id is dropped, and so is a
        // late answer to a call nobody waits for.
        let (ended, replies) = fed(vec![
            text(r#"{"jsonrpc":"2.0","id":7,"method":"x","params":3}"#),
            text(r#"{"jsonrpc":"1.0","method":"x"}"#),
            text(r#"{"jsonrpc":"2.0","id":8,"method":"x"}"#),
            text(r#"{"jsonrpc":"2.0","id":99,"result":{}}"#),
        ])
        .await;
        assert_eq!(ended, Ended::Closed);
        let mut ids: Vec<i64> = replies.iter().map(|r| r["id"].as_i64().unwrap()).collect();
        ids.sort_unstable();
        assert_eq!(ids, [7, 8], "{replies:?}");

        // Too many notifications waiting breaks it too.
        let flood = (0..=MAX_QUEUED_NOTIFICATIONS + 1)
            .map(|_| text(r#"{"jsonrpc":"2.0","method":"slow"}"#))
            .collect();
        let (ended, _) = fed(flood).await;
        assert!(matches!(ended, Ended::Broken(r) if r.contains("notifications")));

        // A call in flight when the other side goes away fails at once,
        // and so does a call made afterwards.
        let (tx, rx) = mpsc::channel(1);
        let (out, mut written) = mpsc::channel(1);
        let (peer, task) = Peer::spawn(rx, out, echoing);
        let waiting = tokio::spawn({
            let peer = peer.clone();
            async move { peer.call_raw("x", json!({})).await }
        });
        written.recv().await.unwrap();
        drop(tx);
        assert_eq!(waiting.await.unwrap(), Err(CallError::Closed));
        assert_eq!(task.await.unwrap(), Ended::Closed);
        assert_eq!(peer.call_raw("x", json!({})).await, Err(CallError::Closed));
        assert_eq!(written.recv().await, None, "the channel is closed");
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn broken_drops_work_in_flight() {
        let (tx, rx) = mpsc::channel(4);
        let (out, mut written) = mpsc::channel(4);
        let (_peer, task) = Peer::spawn(rx, out, echoing);
        tx.send(Ok(br#"{"jsonrpc":"2.0","id":1,"method":"slow"}"#.to_vec()))
            .await
            .unwrap();
        tx.send(Err("the module wrote garbage".into()))
            .await
            .unwrap();
        let ended = tokio::time::timeout(Duration::from_secs(5), task).await;
        assert_eq!(
            ended.unwrap().unwrap(),
            Ended::Broken("the module wrote garbage".into())
        );
        assert_eq!(written.recv().await, None, "the slow request was dropped");
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn closing_answers_what_is_in_flight_then_closes_the_channel() {
        let (tx, rx) = mpsc::channel(4);
        let (out, mut written) = mpsc::channel(4);
        // The handler closes the peer it serves, as a module does when told
        // to shut down; a call it makes afterwards fails at once.
        let closing = |peer: Peer| {
            move |_: String, _: Option<Value>| {
                let peer = peer.clone();
                async move {
                    peer.close();
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    let after = peer.call_raw("x", json!({})).await;
                    Ok(json!({"after": after.is_err()}))
                }
            }
        };
        let (peer, task) = Peer::spawn(rx, out, closing);
        tx.send(Ok(
            br#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#.to_vec()
        ))
        .await
        .unwrap();
        let reply: Value = serde_json::from_slice(&written.recv().await.unwrap()).unwrap();
        assert_eq!(reply["id"], 1, "the shutdown is answered");
        assert_eq!(reply["result"]["after"], true, "{reply}");
        assert_eq!(task.await.unwrap(), Ended::Closed);
        // The other side is still open, but the channel is closed: the
        // writer can finish, and new calls fail at once.
        assert_eq!(written.recv().await, None);
        assert_eq!(peer.call_raw("x", json!({})).await, Err(CallError::Closed));
        drop(tx);
    }
}
