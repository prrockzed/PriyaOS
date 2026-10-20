//! Serving the router on the Unix socket.
//!
//! Each connection gets a [`Session`], a writer task and a reader that
//! splits lines ([`LineSplitter`]) and answers them in order. Only the
//! daemon's own user may connect: anyone else is dropped without an
//! answer. A client that closes its sending side is done: the replies it
//! is owed are sent, and its subscriptions end.
//!
//! When shutdown is signalled, no new connections are taken; each
//! connection finishes the request it is handling (a reply a client won't
//! read is dropped), its subscriptions end with `v1.events.ended`, and it
//! closes. Connections still open after [`DRAIN`] are stopped.

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
/// How long shutdown waits for connections before stopping them.
pub const DRAIN: Duration = Duration::from_secs(5);
/// The pause after a failed accept, so running out of file descriptors
/// doesn't spin.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);
/// What a subscription's client is told when the daemon stops.
const STOPPING: &str = "priyad is stopping.";
/// What it is told when its own connection closes.
const CLOSED: &str = "The connection closed.";

/// Serves `router` on `listener` until `shutdown` becomes true, then waits
/// up to [`DRAIN`] for every connection to close.
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
                Err(e) => {
                    warn!(error = %e, "couldn't accept a connection");
                    tokio::time::sleep(ACCEPT_BACKOFF).await;
                }
            },
            () = stopped(&mut shutdown) => break,
            // Reap finished connections so the set doesn't grow.
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
    drop(listener);
    let drained = tokio::time::timeout(DRAIN, async {
        while connections.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        warn!(
            open = connections.len(),
            "stopping connections that didn't close in time"
        );
        // Wait for the aborted tasks, so nothing still holds the context.
        connections.shutdown().await;
    }
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
    let mut writing = tokio::spawn(async move {
        while let Some(line) = lines.recv().await {
            if writer.write_all(&line).await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    });
    // If this task is stopped from outside, its subscriptions and writer
    // stop with it.
    let cleanup = Cleanup {
        session: session.clone(),
        writer: Some(writing.abort_handle()),
    };

    let mut splitter = LineSplitter::default();
    let mut buffer = vec![0_u8; 64 * 1024];
    'reading: loop {
        tokio::select! {
            biased;
            () = stopped(&mut shutdown) => break,
            read = reader.read(&mut buffer) => {
                let n = match read {
                    Ok(0) => {
                        // A last line without a newline still counts.
                        if let Some(line) = splitter.finish() {
                            answer(&router, &context, &session, line, &mut shutdown).await;
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
                    if !answer(&router, &context, &session, line, &mut shutdown).await {
                        break 'reading;
                    }
                }
            }
        }
    }
    // End the subscriptions, which hold the other senders, then let the
    // writer send what is queued.
    let why = if *shutdown.borrow() { STOPPING } else { CLOSED };
    cleanup.finish(why);
    drop(session);
    if tokio::time::timeout(FLUSH, &mut writing).await.is_err() {
        debug!("a client didn't take its last lines in time");
        writing.abort();
    }
}

/// Ends a connection's subscriptions however the connection ends. If the
/// connection task is stopped from outside (shutdown ran out of time), it
/// also stops the writer, which would otherwise outlive it.
struct Cleanup {
    session: Session,
    writer: Option<tokio::task::AbortHandle>,
}

impl Cleanup {
    /// The connection is closing normally: the writer may finish.
    fn finish(mut self, why: &str) {
        self.writer = None;
        self.session.close(why);
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            self.session.close(STOPPING);
            writer.abort();
        }
    }
}

/// Answers one line; false once the connection is gone or the daemon is
/// stopping before the reply could be queued.
async fn answer<C: Send + Sync + 'static>(
    router: &Router<C>,
    context: &Arc<C>,
    session: &Session,
    line: Line,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    let reply = match line {
        Line::Complete(line) => router.handle(context, session, &line).await,
        Line::TooLong => Some(Router::<C>::too_long()),
    };
    if let Some(reply) = reply {
        // A client that doesn't read mustn't hold up shutdown.
        let sent = tokio::select! {
            // A reply that fits in the queue is still sent while stopping.
            biased;
            sent = session.send_line(reply) => sent,
            () = stopped(shutdown) => false,
        };
        if !sent {
            return false;
        }
    }
    // Subscriptions made by this request may start sending now.
    session.replied();
    true
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    use priya_api::methods::events::{Subscribe, Subscription};
    use priya_api::methods::system::{MethodDescription, Methods, MethodsResult};
    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::unix::OwnedReadHalf;
    use tokio::task::JoinHandle;

    use super::*;

    /// A router whose "subscription" sends one notification at once, and
    /// whose method list is large, to fill a client that doesn't read.
    fn router() -> Router<()> {
        let mut router = Router::default();
        router
            .add::<Methods, _, _>(|_, _, _| async {
                let method = MethodDescription {
                    name: "x".repeat(64 * 1024),
                    summary: String::new(),
                    read_only: true,
                    notification: None,
                    params: Value::Null,
                    result: Value::Null,
                };
                Ok(MethodsResult {
                    methods: vec![method],
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
                            .notify_for(&id, "v1.events.event", &json!({"subscription": id}))
                            .await;
                    }
                });
                session.hold(id.clone(), task.abort_handle());
                Ok(Subscription { subscription: id })
            });
        router
    }

    struct Running {
        _root: tempfile::TempDir,
        path: PathBuf,
        stop: watch::Sender<bool>,
        server: JoinHandle<()>,
    }

    fn start() -> Running {
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
        Running {
            _root: root,
            path,
            stop,
            server,
        }
    }

    impl Running {
        /// Signals shutdown; the server must be gone within `limit`.
        async fn stop_within(self, limit: Duration) {
            self.stop.send(true).unwrap();
            tokio::time::timeout(limit, self.server)
                .await
                .expect("the server stops in time")
                .unwrap();
        }
    }

    async fn next(lines: &mut tokio::io::Lines<BufReader<OwnedReadHalf>>) -> Value {
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap()
    }

    async fn connect(path: &Path) -> UnixStream {
        UnixStream::connect(path).await.unwrap()
    }

    /// The subscriptions told they ended, with the message they got,
    /// until the connection closes.
    async fn ended(
        lines: &mut tokio::io::Lines<BufReader<OwnedReadHalf>>,
    ) -> Vec<(String, String)> {
        let mut ended = Vec::new();
        while let Some(line) = lines.next_line().await.unwrap() {
            let notice: Value = serde_json::from_str(&line).unwrap();
            if notice["method"] == "v1.events.ended" {
                assert_eq!(notice["params"]["reason"], "cancelled");
                ended.push((
                    notice["params"]["subscription"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                    notice["params"]["message"].as_str().unwrap().to_owned(),
                ));
            }
        }
        ended.sort();
        ended
    }

    // covers: CORE-04
    // suite: unit
    #[tokio::test]
    async fn replies_come_before_their_subscriptions_events_and_closing_ends_them() {
        let running = start();
        let (read, mut write) = connect(&running.path).await.into_split();
        let mut lines = BufReader::new(read).lines();
        write
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"v1.events.subscribe\"}\n")
            .await
            .unwrap();
        assert_eq!(
            next(&mut lines).await["result"]["subscription"],
            "sub-1",
            "the reply first"
        );
        assert_eq!(next(&mut lines).await["method"], "v1.events.event");

        // A last request without a newline is answered at end of input;
        // then the client is done and its subscriptions end.
        write
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"v1.events.subscribe\"}")
            .await
            .unwrap();
        write.shutdown().await.unwrap();
        assert_eq!(next(&mut lines).await["id"], 2);
        let closed = CLOSED.to_owned();
        assert_eq!(
            ended(&mut lines).await,
            [
                ("sub-1".to_owned(), closed.clone()),
                ("sub-2".to_owned(), closed)
            ]
        );
        running.stop_within(Duration::from_secs(5)).await;
    }

    // covers: CORE-04
    // suite: unit
    #[tokio::test]
    async fn shutdown_ends_every_subscription_and_closes() {
        let running = start();
        let (read, mut write) = connect(&running.path).await.into_split();
        let mut lines = BufReader::new(read).lines();
        write
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"v1.events.subscribe\"}\n")
            .await
            .unwrap();
        assert_eq!(next(&mut lines).await["id"], 1);
        running.stop_within(Duration::from_secs(5)).await;
        assert_eq!(
            ended(&mut lines).await,
            [("sub-1".to_owned(), STOPPING.to_owned())]
        );
    }

    // covers: CORE-04
    // suite: unit
    #[tokio::test]
    async fn a_connection_stopped_from_outside_stops_its_tasks() {
        let (out, _lines) = mpsc::channel(4);
        let session = Session::new(out);
        let subscription = tokio::spawn(std::future::pending::<()>());
        session.hold(session.new_subscription_id(), subscription.abort_handle());
        let writer = tokio::spawn(std::future::pending::<()>());
        drop(Cleanup {
            session: session.clone(),
            writer: Some(writer.abort_handle()),
        });
        assert!(subscription.await.unwrap_err().is_cancelled());
        assert!(writer.await.unwrap_err().is_cancelled());
        assert_eq!(session.subscription_count(), 0);
    }

    // covers: CORE-04
    // suite: unit
    #[tokio::test]
    async fn a_client_that_never_reads_doesnt_hold_up_shutdown() {
        let running = start();
        let mut stream = connect(&running.path).await;
        // Far more replies than the socket and the queue hold. The writes
        // block too once priyad stops reading, so they go on their own.
        let request = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"v1.system.methods\"}\n";
        let writing = tokio::spawn(async move {
            for _ in 0..(2 * OUTGOING) {
                if stream.write_all(request).await.is_err() {
                    break;
                }
            }
            stream
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        // Well inside DRAIN: the stuck reply is dropped, not waited for.
        running.stop_within(FLUSH + Duration::from_secs(2)).await;
        writing.abort();
    }
}
