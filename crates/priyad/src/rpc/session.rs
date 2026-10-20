//! One connection's side of the conversation: the lines going out to it,
//! and the subscriptions it holds.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use priya_api::framing::encode;
use priya_api::methods::events::{ENDED_NOTIFICATION, EndReason, EndedNotification};
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

    /// Sends a notification for subscription `id`; false once the
    /// connection or the subscription has gone. Nothing is sent after the
    /// reply to an unsubscribe: both take the subscriptions lock, and the
    /// line is queued while it is held.
    pub async fn notify_for(&self, id: &str, method: &str, params: &impl Serialize) -> bool {
        let Some(line) = notification_line(method, params) else {
            return false;
        };
        let Ok(permit) = self.out.reserve().await else {
            return false;
        };
        let subscriptions = self.lock();
        if !subscriptions.contains_key(id) {
            return false;
        }
        permit.send(line);
        true
    }

    /// Sends a line as it is (a response).
    pub(crate) async fn send_line(&self, line: Vec<u8>) -> bool {
        self.out.send(line).await.is_ok()
    }

    /// Resolves once the reply to the request being handled has been sent,
    /// so a subscription's first notification never comes before the
    /// subscription id the client needs to recognise it.
    ///
    /// Only for handlers: the server wakes the waiters after each reply.
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

    /// How many subscriptions are open (answers being sent don't count).
    pub fn subscription_count(&self) -> usize {
        self.lock()
            .keys()
            .filter(|id| !id.starts_with(crate::ai::intents::HOLD_PREFIX))
            .count()
    }

    /// How many held tasks have ids starting with `prefix`.
    pub fn held_with_prefix(&self, prefix: &str) -> usize {
        self.lock()
            .keys()
            .filter(|id| id.starts_with(prefix))
            .count()
    }

    /// Keeps the task feeding subscription `id`, so it can be stopped.
    pub fn hold(&self, id: String, task: AbortHandle) {
        self.lock().insert(id, task);
    }

    /// Stops subscription `id`; false if there is none.
    pub fn cancel(&self, id: &str) -> bool {
        match self.remove(id) {
            Some(task) => {
                task.abort();
                true
            }
            None => false,
        }
    }

    /// Ends subscription `id` from its own task, telling the client why.
    pub async fn end(&self, id: &str, reason: EndReason, message: &str) {
        let ended = EndedNotification {
            subscription: id.to_owned(),
            reason,
            message: message.to_owned(),
        };
        // Sent while still held, so an unsubscribe in between wins.
        self.notify_for(id, ENDED_NOTIFICATION, &ended).await;
        self.remove(id);
    }

    /// Stops every subscription, telling the client each one ended with
    /// `message`. Used when the connection or the daemon closes.
    pub fn close(&self, message: &str) {
        let ended: Vec<String> = {
            let mut subscriptions = self.lock();
            subscriptions
                .drain()
                .map(|(id, task)| {
                    task.abort();
                    id
                })
                .collect()
        };
        // Answers being sent end with their own last step, or are
        // cancelled; only subscriptions get `v1.events.ended`.
        let ended = ended
            .into_iter()
            .filter(|id| !id.starts_with(crate::ai::intents::HOLD_PREFIX));
        for subscription in ended {
            let notice = EndedNotification {
                subscription,
                reason: EndReason::Cancelled,
                message: message.to_owned(),
            };
            // Best effort: a full queue means the client isn't reading.
            if let Some(line) = notification_line(ENDED_NOTIFICATION, &notice) {
                let _ = self.out.try_send(line);
            }
        }
    }

    fn remove(&self, id: &str) -> Option<AbortHandle> {
        self.lock().remove(id)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, AbortHandle>> {
        self.subscriptions.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn notification_line(method: &str, params: &impl Serialize) -> Option<Vec<u8>> {
    let params = serde_json::to_value(params).ok()?;
    encode(&Request::notification(method, params)).ok()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn session() -> (Session, mpsc::Receiver<Vec<u8>>) {
        let (tx, rx) = mpsc::channel(8);
        (Session::new(tx), rx)
    }

    fn read(rx: &mut mpsc::Receiver<Vec<u8>>) -> Option<Value> {
        rx.try_recv()
            .ok()
            .map(|line| serde_json::from_slice(&line).unwrap())
    }

    fn idle_task() -> AbortHandle {
        tokio::spawn(std::future::pending::<()>()).abort_handle()
    }

    // covers: CORE-04
    // suite: unit
    #[tokio::test]
    async fn nothing_is_sent_for_a_cancelled_subscription() {
        let (session, mut rx) = session();
        let id = session.new_subscription_id();
        session.hold(id.clone(), idle_task());
        assert!(session.notify_for(&id, "v1.events.event", &json!({})).await);
        assert_eq!(read(&mut rx).unwrap()["method"], "v1.events.event");

        assert!(session.cancel(&id));
        assert!(!session.cancel(&id), "only once");
        assert!(!session.notify_for(&id, "v1.events.event", &json!({})).await);
        assert_eq!(read(&mut rx), None);
    }

    // covers: CORE-04
    // suite: unit
    #[tokio::test]
    async fn ending_tells_the_client_and_forgets_the_subscription() {
        let (session, mut rx) = session();
        let id = session.new_subscription_id();
        session.hold(id.clone(), idle_task());
        session
            .end(&id, EndReason::Error, "An event couldn't be read.")
            .await;
        let ended = read(&mut rx).unwrap();
        assert_eq!(ended["method"], ENDED_NOTIFICATION);
        assert_eq!(ended["params"]["subscription"], id.as_str());
        assert_eq!(ended["params"]["reason"], "error");
        assert_eq!(session.subscription_count(), 0);
    }

    // covers: CORE-04
    // suite: unit
    #[tokio::test]
    async fn closing_ends_every_subscription_as_cancelled() {
        let (session, mut rx) = session();
        for _ in 0..2 {
            session.hold(session.new_subscription_id(), idle_task());
        }
        session.close("priyad is stopping.");
        let mut ended: Vec<String> = std::iter::from_fn(|| read(&mut rx))
            .map(|n| {
                assert_eq!(n["params"]["reason"], "cancelled");
                n["params"]["subscription"].as_str().unwrap().to_owned()
            })
            .collect();
        ended.sort();
        assert_eq!(ended, ["sub-1", "sub-2"]);
        assert_eq!(session.subscription_count(), 0);
    }
}
