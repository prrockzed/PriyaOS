//! [`Peer`]: one end of a JSON-RPC 2.0 conversation where both sides make
//! calls, as priyad and a module do (02-core §5.2).
//!
//! A peer reads whole messages from a channel and writes them to another,
//! so the same code runs over a process's stdio (with [`lsp`] framing) and
//! over in-process channels (a builtin module). Requests from the other
//! side go to a [`Handler`], several at a time; answers to our own calls go
//! back to whoever is waiting for them.
//!
//! Traffic that isn't JSON-RPC (not JSON, a batch, an answer that can't be
//! read, a message over [`MAX_MESSAGE`]) breaks the conversation: the peer
//! stops and says why ([`Ended::Broken`]), drops the requests it is still
//! handling, and every waiting call gets [`CallError::Closed`]. When the
//! other side just stops sending ([`Ended::Closed`]), the requests it made
//! are still answered first.
//!
//! [`lsp`]: crate::framing::lsp

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::Value;
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio::task::{JoinHandle, JoinSet};

use crate::framing::lsp::MAX_MESSAGE;
use crate::methods::Method;
use crate::rpc::{Id, Outcome, Request, Response, RpcError, parse_request};

/// One message as it arrives: its body, or why the stream broke.
pub type Inbound = Result<Vec<u8>, String>;

/// A boxed future, for [`Handler`].
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The most requests from the other side handled at once; more wait, and
/// the peer stops reading until one finishes.
pub const MAX_IN_FLIGHT: usize = 64;

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
    /// The other side closed it.
    Closed,
    /// The other side broke the protocol; the reason says how.
    Broken(String),
}

/// Our end of the conversation. Cheap to clone; every clone makes calls on
/// the same connection.
#[derive(Clone)]
pub struct Peer {
    shared: Arc<Shared>,
}

struct Shared {
    outbound: mpsc::Sender<Vec<u8>>,
    pending: Mutex<HashMap<i64, oneshot::Sender<Outcome>>>,
    next_id: AtomicI64,
}

impl Shared {
    fn pending(&self) -> std::sync::MutexGuard<'_, HashMap<i64, oneshot::Sender<Outcome>>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    async fn send(&self, message: &impl serde::Serialize) -> Result<(), CallError> {
        let body = serde_json::to_vec(message).map_err(|e| CallError::Mismatch(e.to_string()))?;
        if body.len() > MAX_MESSAGE {
            return Err(CallError::TooLarge(body.len()));
        }
        self.outbound
            .send(body)
            .await
            .map_err(|_| CallError::Closed)
    }
}

impl Peer {
    /// Starts the conversation: reads `inbound`, writes `outbound`, and
    /// hands the other side's requests to `handler`. The task ends when the
    /// conversation does, saying how.
    pub fn spawn(
        inbound: mpsc::Receiver<Inbound>,
        outbound: mpsc::Sender<Vec<u8>>,
        handler: impl Handler,
    ) -> (Self, JoinHandle<Ended>) {
        let shared = Arc::new(Shared {
            outbound,
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
        });
        let task = tokio::spawn(run(shared.clone(), inbound, Arc::new(handler)));
        (Self { shared }, task)
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
        self.shared.pending().insert(id, tx);
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
}

/// Removes a call from the waiting list however its future ends.
struct Forget<'a> {
    shared: &'a Shared,
    id: i64,
}

impl Drop for Forget<'_> {
    fn drop(&mut self) {
        self.shared.pending().remove(&self.id);
    }
}

async fn run(
    shared: Arc<Shared>,
    mut inbound: mpsc::Receiver<Inbound>,
    handler: Arc<dyn Handler>,
) -> Ended {
    let limit = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
    let mut requests = JoinSet::new();
    let ended = loop {
        let body = match inbound.recv().await {
            None => break Ended::Closed,
            Some(Err(reason)) => break Ended::Broken(reason),
            Some(Ok(body)) => body,
        };
        // Reap finished requests so the set doesn't grow.
        while requests.try_join_next().is_some() {}
        if body.len() > MAX_MESSAGE {
            break Ended::Broken(format!(
                "a message of {} bytes is over the {MAX_MESSAGE}-byte limit",
                body.len()
            ));
        }
        let value: Value = match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(e) => break Ended::Broken(format!("a message isn't JSON: {e}")),
        };
        if value.is_array() {
            break Ended::Broken("batches aren't used between priyad and modules".into());
        }
        if value.get("method").is_some() {
            let request = match parse_request(value) {
                Ok(request) => request,
                Err((Id::Null, _)) => continue, // nobody to tell
                Err((id, error)) => {
                    let _ = shared.send(&Response::err(id, error)).await;
                    continue;
                }
            };
            let Ok(permit) = limit.clone().acquire_owned().await else {
                break Ended::Closed;
            };
            let shared = shared.clone();
            let handler = handler.clone();
            requests.spawn(async move {
                let outcome = handler.handle(request.method, request.params).await;
                if let Some(id) = request.id {
                    let response = match outcome {
                        Ok(result) => Response::ok(id.clone(), result),
                        Err(error) => Response::err(id.clone(), error),
                    };
                    if let Err(CallError::TooLarge(_)) = shared.send(&response).await {
                        let error = RpcError::internal("the answer is over the 16 MiB limit");
                        let _ = shared.send(&Response::err(id, error)).await;
                    }
                }
                drop(permit);
            });
            continue;
        }
        let response: Response = match serde_json::from_value(value) {
            Ok(response) => response,
            Err(e) => {
                break Ended::Broken(format!("a message is neither a request nor an answer: {e}"));
            }
        };
        let waiting = match &response.id {
            Id::Number(n) => n.as_i64().and_then(|id| shared.pending().remove(&id)),
            _ => None,
        };
        // An answer nobody waits for (its call timed out) is dropped.
        if let Some(waiting) = waiting {
            let _ = waiting.send(response.outcome);
        }
    };
    // Waiting calls learn the conversation is over: no answer can come.
    shared.pending().clear();
    match ended {
        // The other side stopped sending but may still read: it gets the
        // answers it is owed.
        Ended::Closed => while requests.join_next().await.is_some() {},
        // Nothing it says can be trusted any more.
        Ended::Broken(_) => requests.shutdown().await,
    }
    ended
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;
    use crate::error::{ApiError, ErrorKind};
    use crate::methods::NoParams;
    use crate::module::protocol::{Initialize, InitializeParams, Ping, Shutdown};

    /// Two peers wired to each other, each with its handler.
    fn pair(
        left: impl Handler,
        right: impl Handler,
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
            _ => Ok(json!({"method": method, "params": params})),
        }
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn both_sides_call_each_other_and_errors_keep_their_kind() {
        let ((left, _), (right, _)) = pair(echo, echo);
        let answer = left.call_raw("x.say", json!({"text": "hi"})).await.unwrap();
        assert_eq!(answer, json!({"method": "x.say", "params": {"text": "hi"}}));
        right.call::<Ping>(&NoParams {}).await.unwrap();
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
        left.call::<Shutdown>(&NoParams {}).await.unwrap();
        slow.abort();

        // A call given up on is forgotten; its late answer is ignored.
        let gave_up =
            tokio::time::timeout(Duration::from_millis(20), left.call_raw("slow", json!({}))).await;
        assert!(gave_up.is_err());
        assert!(left.shared.pending().is_empty());

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

    /// A peer fed `bodies` as the other side's messages; what it wrote back.
    async fn fed(bodies: Vec<Inbound>) -> (Ended, Vec<Value>) {
        let (tx, rx) = mpsc::channel(bodies.len() + 1);
        let (out, mut written) = mpsc::channel(16);
        for body in bodies {
            tx.send(body).await.unwrap();
        }
        drop(tx);
        let (_peer, task) = Peer::spawn(rx, out, echo);
        let ended = task.await.unwrap();
        let mut replies = Vec::new();
        while let Ok(body) = written.try_recv() {
            replies.push(serde_json::from_slice(&body).unwrap());
        }
        (ended, replies)
    }

    // covers: CORE-02
    // suite: unit
    #[tokio::test]
    async fn garbage_breaks_the_conversation_and_waiting_calls_learn_it() {
        let broken = |text: &str| Inbound::Ok(text.as_bytes().to_vec());
        for (bodies, why) in [
            (vec![broken("not json")], "isn't JSON"),
            (vec![broken("[]")], "batches"),
            (vec![broken("{\"jsonrpc\":\"2.0\",\"id\":1}")], "neither"),
            (vec![Err("framing broke".into())], "framing broke"),
            (vec![Ok(vec![b' '; MAX_MESSAGE + 1])], "over the"),
        ] {
            let (ended, _) = fed(bodies).await;
            let Ended::Broken(reason) = ended else {
                panic!("broken: {why}");
            };
            assert!(reason.contains(why), "{reason}");
        }

        // A malformed request is answered when it has an id, and the
        // conversation goes on; one without an id is dropped.
        let (ended, replies) = fed(vec![
            broken(r#"{"jsonrpc":"2.0","id":7,"method":"x","params":3}"#),
            broken(r#"{"jsonrpc":"1.0","method":"x"}"#),
            broken(r#"{"jsonrpc":"2.0","id":8,"method":"x"}"#),
            broken(r#"{"jsonrpc":"2.0","id":99,"result":{}}"#),
        ])
        .await;
        assert_eq!(ended, Ended::Closed);
        assert_eq!(replies.len(), 2, "{replies:?}");
        assert_eq!(replies[0]["error"]["code"], -32600);
        assert_eq!(replies[1]["result"]["method"], "x");

        // A call in flight when the other side goes away fails at once.
        let (tx, rx) = mpsc::channel(1);
        let (out, mut written) = mpsc::channel(1);
        let (peer, task) = Peer::spawn(rx, out, echo);
        let waiting = tokio::spawn(async move { peer.call_raw("x", json!({})).await });
        written.recv().await.unwrap();
        drop(tx);
        assert_eq!(waiting.await.unwrap(), Err(CallError::Closed));
        assert_eq!(task.await.unwrap(), Ended::Closed);
    }
}
