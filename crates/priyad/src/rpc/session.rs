//! One connection's side of the conversation: the lines going out to it,
//! and the subscriptions it holds.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use priya_api::framing::encode;
use priya_api::rpc::Request;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot};
use tokio::task::AbortHandle;

/// Cheap to clone; every clone talks to the same connection.
#[derive(Clone)]
pub struct Session {
    out: mpsc::Sender<Vec<u8>>,
    subscriptions: Arc<Mutex<HashMap<String, AbortHandle>>>,
    next: Arc<AtomicU64>,
    /// Woken once the current request's reply is sent.
    after_reply: Arc<Mutex<Vec<oneshot::Sender<()>>>>,
}

impl Session {
    /// `out` carries whole lines to the connection's writer.
    pub fn new(out: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            out,
            subscriptions: Arc::default(),
            next: Arc::default(),
            after_reply: Arc::default(),
        }
    }

    /// Sends a notification; false once the connection has gone.
    pub async fn notify(&self, method: &str, params: &impl Serialize) -> bool {
        let Ok(params) = serde_json::to_value(params) else {
            return false;
        };
        let Ok(line) = encode(&Request::notification(method, params)) else {
            return false;
        };
        self.out.send(line).await.is_ok()
    }

    /// Sends a line as it is (a response).
    pub(crate) async fn send_line(&self, line: Vec<u8>) -> bool {
        self.out.send(line).await.is_ok()
    }

    /// Resolves once the reply to the request being handled has been sent,
    /// so a subscription's first notification never comes before the
    /// subscription id the client needs to recognise it.
    pub fn after_reply(&self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.after_reply
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(tx);
        rx
    }

    /// Called by the server once a reply is on its way.
    pub(crate) fn replied(&self) {
        let waiting =
            std::mem::take(&mut *self.after_reply.lock().unwrap_or_else(|e| e.into_inner()));
        for waiter in waiting {
            let _ = waiter.send(());
        }
    }

    /// A new subscription id, unique on this connection.
    pub fn new_subscription_id(&self) -> String {
        format!("sub-{}", self.next.fetch_add(1, Ordering::Relaxed) + 1)
    }

    /// Keeps the task feeding subscription `id`, so it can be stopped.
    pub fn hold(&self, id: String, task: AbortHandle) {
        self.lock().insert(id, task);
    }

    /// Stops subscription `id`; false if there is none.
    pub fn cancel(&self, id: &str) -> bool {
        match self.lock().remove(id) {
            Some(task) => {
                task.abort();
                true
            }
            None => false,
        }
    }

    /// Stops every subscription, when the connection ends.
    pub fn close(&self) {
        for (_, task) in self.lock().drain() {
            task.abort();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, AbortHandle>> {
        self.subscriptions.lock().unwrap_or_else(|e| e.into_inner())
    }
}
