//! Serving the router on the Unix socket.
//!
//! Each connection gets a [`Session`], a writer task and a reader that
//! splits lines ([`LineSplitter`]) and answers them in order. Only the
//! daemon's own user may connect: anyone else is dropped without an
//! answer. When shutdown is signalled, no new connections are taken; each
//! connection finishes the request it is handling, its subscriptions stop,
//! and it closes.

use std::sync::Arc;
use std::time::Duration;

use priya_api::framing::{Line, LineSplitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tracing::{debug, warn};

use super::{Router, Session};

/// Lines waiting for a slow client before the connection stops reading.
const OUTGOING: usize = 256;
/// How long a closing connection may take to send what it has.
const FLUSH: Duration = Duration::from_secs(2);

/// Serves `router` on `listener` until `shutdown` becomes true, then waits
/// for every connection to close.
pub async fn serve<C: Send + Sync + 'static>(
    listener: UnixListener,
    own_uid: u32,
    router: Arc<Router<C>>,
    context: Arc<C>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    if !same_user(&stream, own_uid) {
                        continue;
                    }
                    connections.spawn(connection(stream, router.clone(), context.clone(), shutdown.clone()));
                }
                Err(e) => warn!(error = %e, "couldn't accept a connection"),
            },
            () = stopped(&mut shutdown) => break,
            // Reap finished connections so the set doesn't grow.
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
    while connections.join_next().await.is_some() {}
}

/// Resolves once shutdown is signalled (or its sender is gone).
async fn stopped(shutdown: &mut watch::Receiver<bool>) {
    let _ = shutdown.wait_for(|stop| *stop).await;
}

/// Whether the peer runs as the daemon's own user.
fn same_user(stream: &UnixStream, own_uid: u32) -> bool {
    match stream.peer_cred() {
        Ok(peer) if peer.uid() == own_uid => true,
        Ok(peer) => {
            warn!(uid = peer.uid(), "refused a connection from another user");
            false
        }
        Err(e) => {
            warn!(error = %e, "refused a connection whose user can't be checked");
            false
        }
    }
}

async fn connection<C: Send + Sync + 'static>(
    stream: UnixStream,
    router: Arc<Router<C>>,
    context: Arc<C>,
    mut shutdown: watch::Receiver<bool>,
) {
    let (mut reader, mut writer) = stream.into_split();
    let (out, mut lines) = mpsc::channel::<Vec<u8>>(OUTGOING);
    let session = Session::new(out);
    let writing = tokio::spawn(async move {
        while let Some(line) = lines.recv().await {
            if writer.write_all(&line).await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    });

    let mut splitter = LineSplitter::default();
    let mut buffer = vec![0_u8; 64 * 1024];
    'reading: loop {
        tokio::select! {
            read = reader.read(&mut buffer) => {
                let n = match read {
                    Ok(0) => {
                        // A last line without a newline still counts.
                        if let Some(line) = splitter.finish() {
                            answer(&router, &context, &session, line).await;
                        }
                        break;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        debug!(error = %e, "connection read failed");
                        break;
                    }
                };
                for line in splitter.push(&buffer[..n]) {
                    if !answer(&router, &context, &session, line).await {
                        break 'reading;
                    }
                }
            }
            () = stopped(&mut shutdown) => break,
        }
    }
    // Stop the subscriptions, which hold the other senders, then let the
    // writer send what is queued.
    session.close();
    drop(session);
    let _ = tokio::time::timeout(FLUSH, writing).await;
}

/// Answers one line; false once the connection is gone.
async fn answer<C: Send + Sync + 'static>(
    router: &Router<C>,
    context: &Arc<C>,
    session: &Session,
    line: Line,
) -> bool {
    let reply = match line {
        Line::Complete(line) => router.handle(context, session, &line).await,
        Line::TooLong => Some(Router::<C>::too_long()),
    };
    if let Some(reply) = reply
        && !session.send_line(reply).await
    {
        return false;
    }
    // Subscriptions made by this request may start sending now.
    session.replied();
    true
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use priya_api::methods::events::{Subscribe, Subscription};
    use priya_api::methods::system::{Methods, MethodsResult};
    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    use super::*;

    /// A router whose "subscription" sends one notification at once.
    fn router() -> Router<()> {
        let mut router = Router::default();
        router
            .add::<Methods, _, _>(|_, _, _| async {
                Ok(MethodsResult {
                    methods: Vec::new(),
                })
            })
            .add::<Subscribe, _, _>(|_, _, session| async move {
                let id = session.new_subscription_id();
                let replied = session.after_reply();
                let task = tokio::spawn({
                    let session = session.clone();
                    let id = id.clone();
                    async move {
                        let _ = replied.await;
                        session
                            .notify("v1.events.event", &json!({"subscription": id}))
                            .await;
                    }
                });
                session.hold(id.clone(), task.abort_handle());
                Ok(Subscription { subscription: id })
            });
        router
    }

    // covers: CORE-04
    // suite: unit
    #[tokio::test]
    async fn replies_come_before_their_subscriptions_events_and_shutdown_closes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let uid = std::fs::metadata(&path).unwrap().uid();
        let (stop, stopping) = watch::channel(false);
        let server = tokio::spawn(serve(
            listener,
            uid,
            Arc::new(router()),
            Arc::new(()),
            stopping,
        ));

        let stream = UnixStream::connect(&path).await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        write
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"v1.events.subscribe\"}\n")
            .await
            .unwrap();
        let first: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(first["result"]["subscription"], "sub-1", "the reply first");
        let second: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(second["method"], "v1.events.event");

        // A last request without a newline is answered at end of input.
        write
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"v1.system.methods\"}")
            .await
            .unwrap();
        write.shutdown().await.unwrap();
        let third: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(third["id"], 2);

        stop.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("the server stops")
            .unwrap();
    }
}
