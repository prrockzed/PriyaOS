//! `mock-priyad`: a stand-in for priyad that plays a script, for UI tests
//! (the Priya bar's behaviour tests and journeys, 0.13). It speaks the real
//! wire format (newline-delimited JSON-RPC on a Unix socket) but answers
//! from a JSON script instead of running anything:
//!
//! ```json
//! {
//!   "on_connect": [{"delay_ms": 0, "method": "v1.events.event", "params": {}}],
//!   "methods": {
//!     "v1.settings.get": [{"result": {"value": "dark"}}],
//!     "v1.intent.submit": [{
//!       "result": {"intent_id": "i1", "conversation_id": "c1"},
//!       "notify": [
//!         {"delay_ms": 300, "method": "v1.intent.stream", "params": {"intent_id": "i1", "seq": 1, "kind": "started", "model": "qwen2.5:0.5b"}},
//!         {"delay_ms": 100, "method": "v1.intent.stream", "params": {"intent_id": "i1", "seq": 2, "kind": "delta", "text": "Hi"}}
//!       ]
//!     }],
//!     "v1.intent.cancel": [{"result": {}, "stop": "v1.intent.submit", "notify": [...]}]
//!   }
//! }
//! ```
//!
//! Each method's replies are used in order, the last one again once they
//! run out; a reply with `params` only answers requests whose params
//! contain those values (`{"key": "ui.theme"}`), so one method can answer
//! differently by request. A method the script doesn't name gets "method
//! not found", and lines that aren't valid requests get the error priyad
//! would send.
//!
//! `notify` steps are sent after the reply, each `delay_ms` after the one
//! before: to the asking connection, or to every connection with
//! `broadcast` (a `v1.settings.set` from a test reaching the bar's event
//! subscription, as in priyad). `stop` names a method whose notifications
//! still waiting on this connection are dropped (a cancel stops the
//! answer's stream). `close` hangs up after the reply, for testing a
//! client's reconnect.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use priya_api::framing::{Line, LineSplitter, encode};
use priya_api::rpc::{Id, Incoming, Request, Response, RpcError, parse_line, parse_request};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::lock;

/// What mock-priyad does.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Script {
    /// Notifications sent to each new connection.
    pub on_connect: Vec<Notify>,
    /// Replies by method name.
    pub methods: BTreeMap<String, Vec<Reply>>,
}

/// One reply to a request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reply {
    /// Answer only requests whose params contain these values.
    pub params: Option<Value>,
    pub result: Option<Value>,
    /// An `ApiError` (`kind`, `retryable`, `user_message`), sent as the
    /// error's data; wins over `result`.
    pub error: Option<Value>,
    /// Sent after the reply.
    pub notify: Vec<Notify>,
    /// Send `notify` to every connection, not just the asking one.
    pub broadcast: bool,
    /// Drop this connection's waiting notifications that this method's
    /// replies started.
    pub stop: Option<String>,
    /// Hang up after replying.
    pub close: bool,
}

/// One notification, sent `delay_ms` after the one before it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Notify {
    pub delay_ms: u64,
    pub method: String,
    pub params: Value,
}

impl Script {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// The replies left, shared by every connection.
struct Replies(Mutex<BTreeMap<String, Vec<Reply>>>);

impl Replies {
    /// The first reply to `method` whose `params` match the request's.
    /// Replies with the same `params` form a queue: each is used once,
    /// except the last, which stays.
    fn next(&self, method: &str, params: &Value) -> Option<Reply> {
        let mut replies = lock(&self.0);
        let queue = replies.get_mut(method)?;
        let first = queue.iter().position(|reply| {
            reply
                .params
                .as_ref()
                .is_none_or(|want| contains(params, want))
        })?;
        let pattern = queue[first].params.clone();
        let same = queue.iter().filter(|reply| reply.params == pattern).count();
        Some(if same > 1 {
            queue.remove(first)
        } else {
            queue[first].clone()
        })
    }
}

/// Whether `value` has everything `want` has (objects compared key by key,
/// anything else equal).
fn contains(value: &Value, want: &Value) -> bool {
    match (value, want) {
        (Value::Object(value), Value::Object(want)) => want
            .iter()
            .all(|(key, want)| value.get(key).is_some_and(|value| contains(value, want))),
        _ => value == want,
    }
}

/// Every open connection's outgoing line queue, for broadcasts.
#[derive(Default)]
struct Connections(Mutex<Vec<mpsc::Sender<Vec<u8>>>>);

impl Connections {
    fn add(&self, out: mpsc::Sender<Vec<u8>>) {
        let mut all = lock(&self.0);
        all.retain(|out| !out.is_closed());
        all.push(out);
    }

    fn all(&self) -> Vec<mpsc::Sender<Vec<u8>>> {
        lock(&self.0).clone()
    }
}

/// Serves `script` on `listener` until the task is stopped or accepting
/// fails (the error is returned).
pub async fn serve(listener: UnixListener, script: Script) -> std::io::Result<()> {
    let replies = Arc::new(Replies(Mutex::new(script.methods)));
    let on_connect = Arc::new(script.on_connect);
    let everyone = Arc::new(Connections::default());
    let mut connections = JoinSet::new();
    loop {
        let (stream, _) = listener.accept().await?;
        connections.spawn(connection(
            stream,
            replies.clone(),
            on_connect.clone(),
            everyone.clone(),
        ));
        while connections.try_join_next().is_some() {}
    }
}

async fn connection(
    stream: UnixStream,
    replies: Arc<Replies>,
    on_connect: Arc<Vec<Notify>>,
    everyone: Arc<Connections>,
) {
    let (mut reader, mut writer) = stream.into_split();
    let (out, mut lines) = mpsc::channel::<Vec<u8>>(256);
    everyone.add(out.clone());
    let writing = tokio::spawn(async move {
        while let Some(line) = lines.recv().await {
            if writer.write_all(&line).await.is_err() {
                break;
            }
        }
    });
    // Waiting notifications, by the method whose reply started them.
    let mut pending: HashMap<String, JoinSet<()>> = HashMap::new();
    start(&mut pending, "", vec![out.clone()], on_connect.to_vec());
    let mut splitter = LineSplitter::default();
    let mut buffer = vec![0_u8; 64 * 1024];
    'reading: loop {
        let n = match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for line in splitter.push(&buffer[..n]) {
            let (response, reply, method) = respond(&replies, line);
            if let Some(line) = response.and_then(|r| encode(&r).ok()) {
                let _ = out.send(line).await;
            }
            let Some(reply) = reply else { continue };
            if let Some(set) = reply.stop.as_ref().and_then(|m| pending.get_mut(m)) {
                set.abort_all();
            }
            let targets = if reply.broadcast {
                everyone.all()
            } else {
                vec![out.clone()]
            };
            start(&mut pending, &method, targets, reply.notify);
            if reply.close {
                break 'reading;
            }
        }
    }
    for set in pending.values_mut() {
        set.abort_all();
    }
    drop(out);
    let _ = writing.await;
}

/// Sends `steps` later, filed under `method` so a scripted `stop` can
/// drop them.
fn start(
    pending: &mut HashMap<String, JoinSet<()>>,
    method: &str,
    targets: Vec<mpsc::Sender<Vec<u8>>>,
    steps: Vec<Notify>,
) {
    let set = pending.entry(method.to_owned()).or_default();
    while set.try_join_next().is_some() {}
    set.spawn(send_later(targets, steps));
}

/// The response to one line (none for a client notification), the
/// scripted reply used, and the method asked.
fn respond(replies: &Replies, line: Line) -> (Option<Response>, Option<Reply>, String) {
    let line = match line {
        Line::Complete(line) => line,
        Line::TooLong => {
            let error = RpcError::invalid_request("the request is longer than 16 MiB");
            return (Some(Response::err(Id::Null, error)), None, String::new());
        }
    };
    let message = match parse_line(&line) {
        Ok(Incoming::Single(message)) => message,
        Ok(Incoming::Batch(_)) => {
            let error = RpcError::invalid_request("mock-priyad doesn't take batches");
            return (Some(Response::err(Id::Null, error)), None, String::new());
        }
        Err(error) => return (Some(Response::err(Id::Null, error)), None, String::new()),
    };
    let request = match parse_request(message) {
        Ok(request) => request,
        Err((id, error)) => return (Some(Response::err(id, error)), None, String::new()),
    };
    let method = request.method.clone();
    let params = request.params.clone().unwrap_or(Value::Null);
    let reply = replies.next(&method, &params);
    // Notifications from the client get no answer, scripted or not.
    let Some(id) = request.id else {
        return (None, reply, method);
    };
    let Some(reply) = reply else {
        return (
            Some(Response::err(id, RpcError::method_not_found(&method))),
            None,
            method,
        );
    };
    let response = match &reply.error {
        Some(data) => match serde_json::from_value(data.clone()) {
            Ok(error) => Response::err(id, RpcError::from_api(error)),
            Err(e) => Response::err(id, RpcError::internal(format!("bad scripted error: {e}"))),
        },
        None => Response::ok(id, reply.result.clone().unwrap_or(Value::Null)),
    };
    (Some(response), Some(reply), method)
}

async fn send_later(targets: Vec<mpsc::Sender<Vec<u8>>>, steps: Vec<Notify>) {
    for step in steps {
        tokio::time::sleep(Duration::from_millis(step.delay_ms)).await;
        let Ok(line) = encode(&Request::notification(&step.method, step.params)) else {
            continue;
        };
        for out in &targets {
            // A connection that has gone just misses it.
            let _ = out.send(line.clone()).await;
        }
    }
}
