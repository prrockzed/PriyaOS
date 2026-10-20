//! [`ScriptedHttp`]: a local HTTP server that plays scripted replies, for
//! testing HTTP adapters without a network: statuses, headers, a streamed
//! body with pauses, or a body that never ends.
//!
//! It records each request and counts open connections. While it writes a
//! reply it also watches the socket, so a client that hangs up is noticed
//! at once rather than at the next write; that is what lets a test check a
//! cancel closes the connection within 100 ms.
//!
//! wiremock was considered: it delays whole responses but can't pace a
//! streamed body, which the timeout and cancel tests need.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use crate::lock;

/// One part of a reply body.
#[derive(Debug, Clone)]
pub enum BodyStep {
    Send(Vec<u8>),
    Pause(Duration),
    /// Keep the connection open and send nothing more.
    Hang,
}

/// What the server answers to one request.
#[derive(Debug, Clone)]
pub struct HttpReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<BodyStep>,
}

impl HttpReply {
    /// A status with a plain body.
    pub fn status(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: vec![BodyStep::Send(body.as_bytes().to_vec())],
        }
    }

    /// A 200 event stream: each item is sent as `data: <item>` and a blank
    /// line, in one piece.
    pub fn sse(events: &[&str]) -> Self {
        Self::sse_steps(
            events
                .iter()
                .map(|event| BodyStep::Send(format!("data: {event}\n\n").into_bytes()))
                .collect(),
        )
    }

    /// A 200 event stream made of these steps, written as they are.
    pub fn sse_steps(body: Vec<BodyStep>) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            body,
        }
    }

    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// A request the server received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    /// Names in lower case.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl RecordedRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Default)]
struct Shared {
    replies: Mutex<VecDeque<HttpReply>>,
    requests: Mutex<Vec<RecordedRequest>>,
    open: AtomicUsize,
}

/// The server; it stops when dropped.
pub struct ScriptedHttp {
    addr: SocketAddr,
    shared: Arc<Shared>,
    accept: JoinHandle<()>,
}

impl ScriptedHttp {
    /// Starts on a free local port. Replies are used in order; a request
    /// with none left gets a 500 that says so.
    pub async fn start(replies: Vec<HttpReply>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let shared = Arc::new(Shared {
            replies: Mutex::new(replies.into()),
            ..Shared::default()
        });
        let accept = tokio::spawn({
            let shared = shared.clone();
            async move {
                while let Ok((socket, _)) = listener.accept().await {
                    tokio::spawn(serve(socket, shared.clone()));
                }
            }
        });
        Ok(Self {
            addr,
            shared,
            accept,
        })
    }

    /// `http://127.0.0.1:<port>/v1`, as an OpenAI-style base URL.
    pub fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        lock(&self.shared.requests).clone()
    }

    /// Connections the server is still writing to.
    pub fn open(&self) -> usize {
        self.shared.open.load(Ordering::SeqCst)
    }
}

impl Drop for ScriptedHttp {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

async fn serve(mut socket: TcpStream, shared: Arc<Shared>) {
    let Some(request) = read_request(&mut socket).await else {
        return;
    };
    lock(&shared.requests).push(request);
    let reply = lock(&shared.replies)
        .pop_front()
        .unwrap_or_else(|| HttpReply::status(500, r#"{"error":"the script has no reply left"}"#));
    shared.open.fetch_add(1, Ordering::SeqCst);
    let (mut reader, mut writer) = socket.into_split();
    let mut sink = [0_u8; 512];
    // Writing stops as soon as the client hangs up (its read side ends).
    tokio::select! {
        () = write_reply(&mut writer, &reply) => {}
        _ = reader.read(&mut sink) => {}
    }
    shared.open.fetch_sub(1, Ordering::SeqCst);
}

async fn write_reply(writer: &mut tokio::net::tcp::OwnedWriteHalf, reply: &HttpReply) {
    let mut head = format!(
        "HTTP/1.1 {} Scripted\r\nconnection: close\r\n",
        reply.status
    );
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    if writer.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    for step in &reply.body {
        match step {
            BodyStep::Send(bytes) => {
                if writer.write_all(bytes).await.is_err() || writer.flush().await.is_err() {
                    return;
                }
            }
            BodyStep::Pause(pause) => tokio::time::sleep(*pause).await,
            BodyStep::Hang => std::future::pending::<()>().await,
        }
    }
    let _ = writer.shutdown().await;
}

/// Reads one request: the head up to the blank line, then
/// `content-length` bytes of body.
async fn read_request(socket: &mut TcpStream) -> Option<RecordedRequest> {
    let mut data = Vec::new();
    let mut buffer = [0_u8; 4096];
    let head_end = loop {
        if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break end;
        }
        let n = socket.read(&mut buffer).await.ok()?;
        if n == 0 {
            return None;
        }
        data.extend_from_slice(&buffer[..n]);
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let path = first.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = data[head_end + 4..].to_vec();
    while body.len() < length {
        let n = socket.read(&mut buffer).await.ok()?;
        if n == 0 {
            return None;
        }
        body.extend_from_slice(&buffer[..n]);
    }
    Some(RecordedRequest {
        method,
        path,
        headers,
        body,
    })
}
