//! `Content-Length` framing, as the Language Server Protocol uses: each
//! message is a header block and a JSON body of exactly that many bytes.
//!
//! ```text
//! Content-Length: 17\r\n
//! \r\n
//! {"jsonrpc":"2.0"}
//! ```
//!
//! This is how priyad and process modules talk over stdio (02-core §5.2).
//! Bodies are at most [`MAX_MESSAGE`] bytes. `Content-Type` and any other
//! header are ignored. A stream that breaks the framing can't be read past
//! the break, so [`Decoder`] stays failed once it reports an error.

use serde::Serialize;

use super::{EncodeError, MAX_LINE};

/// The largest body accepted: 16 MiB, as on the surfaces' socket.
pub const MAX_MESSAGE: usize = MAX_LINE;

/// The longest header block accepted, in bytes.
pub const MAX_HEADERS: usize = 4096;

const END_OF_HEADERS: &[u8] = b"\r\n\r\n";

/// Why a stream couldn't be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    /// The header block is malformed or has no usable `Content-Length`.
    #[error("bad message header: {0}")]
    Header(String),
    #[error("a message of {length} bytes is over the {max}-byte limit")]
    TooLong { length: usize, max: usize },
    #[error("the message header is over {MAX_HEADERS} bytes")]
    HeadersTooLong,
    /// The stream ended inside a message.
    #[error("the stream ended in the middle of a message")]
    Truncated,
}

/// Turns a byte stream into message bodies, however the bytes arrive.
#[derive(Debug)]
pub struct Decoder {
    /// Bytes not yet part of a returned body.
    buffer: Vec<u8>,
    max: usize,
    /// The length of the body being read, once its header is in.
    body: Option<usize>,
    failed: Option<FrameError>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new(MAX_MESSAGE)
    }
}

impl Decoder {
    /// A decoder that refuses bodies over `max` bytes.
    pub fn new(max: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max,
            body: None,
            failed: None,
        }
    }

    /// Adds `bytes` and returns the bodies they complete, or the error that
    /// broke the stream (again on every later call). Bodies completed before
    /// a break are returned first, with the break in [`error`](Self::error):
    /// a caller that gets bodies checks `error()` before reading on.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        self.buffer.extend_from_slice(bytes);
        let mut bodies = Vec::new();
        // Everything before `used` is taken; it is dropped once, at the
        // end, so many small messages in one read cost one copy.
        let mut used = 0;
        let outcome = loop {
            let rest = &self.buffer[used..];
            match self.body {
                Some(length) if rest.len() >= length => {
                    bodies.push(rest[..length].to_vec());
                    used += length;
                    self.body = None;
                }
                Some(_) => break Ok(()),
                None => match header(rest, self.max) {
                    Ok(Some((taken, length))) => {
                        used += taken;
                        self.body = Some(length);
                    }
                    Ok(None) => break Ok(()),
                    Err(error) => break Err(error),
                },
            }
        };
        match outcome {
            Ok(()) => {
                self.buffer.drain(..used);
                Ok(bodies)
            }
            Err(error) => {
                self.buffer = Vec::new();
                self.failed = Some(error.clone());
                // Whole messages before the break still count; the break
                // shows in `error()` and on the next call.
                if bodies.is_empty() {
                    Err(error)
                } else {
                    Ok(bodies)
                }
            }
        }
    }

    /// The error that broke the stream, if it is broken.
    pub fn error(&self) -> Option<&FrameError> {
        self.failed.as_ref()
    }

    /// At end of input: an error if it stopped inside a message.
    pub fn finish(&self) -> Result<(), FrameError> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if self.body.is_some() || !self.buffer.is_empty() {
            return Err(FrameError::Truncated);
        }
        Ok(())
    }
}

/// Reads a header block at the start of `bytes`: how many bytes it takes
/// and its `Content-Length`; none while it is incomplete.
fn header(bytes: &[u8], max: usize) -> Result<Option<(usize, usize)>, FrameError> {
    // Only the first MAX_HEADERS bytes and the blank line may be header,
    // however the bytes were split into reads.
    let window = &bytes[..bytes.len().min(MAX_HEADERS + END_OF_HEADERS.len())];
    let end = window
        .windows(END_OF_HEADERS.len())
        .position(|w| w == END_OF_HEADERS);
    // Body bytes may hold anything; only the header is checked.
    let head = &window[..end.unwrap_or(window.len())];
    let bare_newline = head
        .iter()
        .enumerate()
        .any(|(i, b)| *b == b'\n' && (i == 0 || head[i - 1] != b'\r'));
    if bare_newline {
        return Err(FrameError::Header(
            "header lines must end in \\r\\n, not just \\n".into(),
        ));
    }
    let Some(end) = end else {
        return if window.len() == MAX_HEADERS + END_OF_HEADERS.len() {
            Err(FrameError::HeadersTooLong)
        } else {
            Ok(None)
        };
    };
    let block = std::str::from_utf8(&bytes[..end])
        .map_err(|_| FrameError::Header("it isn't text".into()))?;
    let length = content_length(block)?;
    if length > max {
        return Err(FrameError::TooLong { length, max });
    }
    Ok(Some((end + END_OF_HEADERS.len(), length)))
}

/// The one `Content-Length` in a header block.
fn content_length(block: &str) -> Result<usize, FrameError> {
    let mut length = None;
    for line in block.split("\r\n") {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| FrameError::Header(format!("`{}` isn't a header", shorten(line))))?;
        if !name.trim().eq_ignore_ascii_case("content-length") {
            continue;
        }
        let value = value.trim();
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(FrameError::Header(format!(
                "Content-Length `{}` isn't a number",
                shorten(value)
            )));
        }
        if length.is_some() {
            return Err(FrameError::Header("Content-Length is given twice".into()));
        }
        // Too many digits for usize is certainly too long.
        length = Some(value.parse().unwrap_or(usize::MAX));
    }
    length.ok_or_else(|| FrameError::Header("there's no Content-Length".into()))
}

/// At most 40 characters of `text`, for messages.
fn shorten(text: &str) -> String {
    text.chars().take(40).collect()
}

/// `value` as compact JSON in one framed message, if it fits.
pub fn encode(value: &impl Serialize) -> Result<Vec<u8>, EncodeError> {
    encode_body(&serde_json::to_vec(value)?)
}

/// `body` framed, if it fits.
pub fn encode_body(body: &[u8]) -> Result<Vec<u8>, EncodeError> {
    if body.len() > MAX_MESSAGE {
        return Err(EncodeError::TooLong(body.len()));
    }
    let mut message = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    message.extend_from_slice(body);
    Ok(message)
}

/// Reads framed messages from `reader` into `inbound` until the stream
/// ends or breaks; a break is sent as the last item, with its reason.
#[cfg(feature = "peer")]
pub async fn read_messages<R>(
    mut reader: R,
    inbound: tokio::sync::mpsc::Sender<crate::peer::Inbound>,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;
    let mut decoder = Decoder::default();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(0) => decoder.finish().map(|()| None),
            Ok(n) => decoder.push(&buffer[..n]).map(Some),
            Err(e) => {
                let _ = inbound.send(Err(format!("reading failed: {e}"))).await;
                return;
            }
        };
        match read {
            Ok(None) => return,
            Ok(Some(bodies)) => {
                for body in bodies {
                    if inbound.send(Ok(body)).await.is_err() {
                        return;
                    }
                }
                if let Some(error) = decoder.error() {
                    let _ = inbound.send(Err(error.to_string())).await;
                    return;
                }
            }
            Err(e) => {
                let _ = inbound.send(Err(e.to_string())).await;
                return;
            }
        }
    }
}

/// [`read_messages`] for a blocking reader, on a thread of its own. A
/// process module reads stdin this way: tokio's stdin can't be stopped, so
/// a runtime waiting for it would never shut down.
#[cfg(feature = "peer")]
pub fn read_messages_blocking<R>(
    mut reader: R,
    inbound: tokio::sync::mpsc::Sender<crate::peer::Inbound>,
) where
    R: std::io::Read,
{
    let mut decoder = Decoder::default();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => decoder.finish().map(|()| None),
            Ok(n) => decoder.push(&buffer[..n]).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ = inbound.blocking_send(Err(format!("reading failed: {e}")));
                return;
            }
        };
        match read {
            Ok(None) => return,
            Ok(Some(bodies)) => {
                for body in bodies {
                    if inbound.blocking_send(Ok(body)).is_err() {
                        return;
                    }
                }
                if let Some(error) = decoder.error() {
                    let _ = inbound.blocking_send(Err(error.to_string()));
                    return;
                }
            }
            Err(e) => {
                let _ = inbound.blocking_send(Err(e.to_string()));
                return;
            }
        }
    }
}

/// Writes each message from `outbound` to `writer`, framed, until the
/// channel closes (then shuts the writer, so the other side sees the end)
/// or writing fails, which it returns.
#[cfg(feature = "peer")]
pub async fn write_messages<W>(
    mut writer: W,
    mut outbound: tokio::sync::mpsc::Receiver<Vec<u8>>,
) -> std::io::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;
    while let Some(body) = outbound.recv().await {
        // The peer refuses oversize messages before they get here, so this
        // is a bug, not something to skip quietly.
        let message = encode_body(&body).map_err(std::io::Error::other)?;
        writer.write_all(&message).await?;
        writer.flush().await?;
    }
    writer.shutdown().await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn framed(body: &str) -> Vec<u8> {
        encode_body(body.as_bytes()).unwrap()
    }

    // covers: CORE-02
    // suite: unit
    // acceptance: 0.8
    #[test]
    fn lsp_framing_round_trip_and_oversize_rejected() {
        let message = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
        let bytes = encode(&message).unwrap();
        let body = serde_json::to_vec(&message).unwrap();
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        assert_eq!(bytes, [header.as_bytes(), &body].concat());

        // Whole, two at once, and one byte at a time all give the bodies.
        let mut decoder = Decoder::default();
        let bodies = decoder
            .push(&[bytes.clone(), bytes.clone()].concat())
            .unwrap();
        assert_eq!(bodies.len(), 2);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bodies[1]).unwrap(),
            message
        );
        let mut decoder = Decoder::default();
        let mut bodies = Vec::new();
        for byte in &bytes {
            bodies.extend(decoder.push(std::slice::from_ref(byte)).unwrap());
        }
        assert_eq!(bodies, [serde_json::to_vec(&message).unwrap()]);
        decoder.finish().unwrap();

        // Other headers, any case and spacing; an empty body is a body.
        let mut decoder = Decoder::default();
        let headers = b"content-type: application/vscode-jsonrpc; charset=utf-8\r\nCONTENT-LENGTH:  2 \r\n\r\n{}";
        assert_eq!(decoder.push(headers).unwrap(), [b"{}".to_vec()]);
        assert_eq!(decoder.push(&framed("")).unwrap(), [Vec::<u8>::new()]);

        // Exactly the limit is fine; one byte more is refused before the
        // body is read, and the stream stays broken.
        let mut decoder = Decoder::new(4);
        assert_eq!(decoder.push(&framed("1234")).unwrap(), [b"1234".to_vec()]);
        let five = FrameError::TooLong { length: 5, max: 4 };
        assert_eq!(
            decoder.push(b"Content-Length: 5\r\n\r\n"),
            Err(five.clone())
        );
        assert_eq!(
            five.to_string(),
            "a message of 5 bytes is over the 4-byte limit"
        );
        assert_eq!(decoder.push(&framed("ok")), Err(five.clone()));
        assert_eq!(decoder.finish(), Err(five));
        let mut decoder = Decoder::default();
        let huge = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE + 1);
        assert!(matches!(
            decoder.push(huge.as_bytes()),
            Err(FrameError::TooLong { length, .. }) if length == MAX_MESSAGE + 1
        ));
        let mut decoder = Decoder::default();
        assert!(matches!(
            decoder.push(b"Content-Length: 99999999999999999999999\r\n\r\n"),
            Err(FrameError::TooLong {
                length: usize::MAX,
                ..
            })
        ));

        // Many small messages in one read cost one copy, and each body
        // holds only its own bytes.
        let many = framed("{}").repeat(10_000);
        let bodies = Decoder::default().push(&many).unwrap();
        assert_eq!(bodies.len(), 10_000);
        assert!(bodies.iter().map(Vec::capacity).sum::<usize>() < 100_000);
        // A body may contain anything, newlines included.
        let mut decoder = Decoder::default();
        assert_eq!(decoder.push(&framed("{\n}")).unwrap(), [b"{\n}".to_vec()]);

        // The encoder refuses what the decoder would.
        let big = vec![b' '; MAX_MESSAGE];
        assert_eq!(
            encode_body(&big).unwrap().len(),
            MAX_MESSAGE + "Content-Length: 16777216\r\n\r\n".len()
        );
        let too_big = vec![b' '; MAX_MESSAGE + 1];
        assert!(matches!(
            encode_body(&too_big),
            Err(EncodeError::TooLong(_))
        ));
    }

    // covers: CORE-02
    // suite: unit
    #[test]
    fn lsp_framing_refuses_broken_headers_and_cut_streams() {
        let refused = |bytes: &[u8]| Decoder::default().push(bytes).unwrap_err();
        assert!(
            matches!(refused(b"garbage\r\n\r\n"), FrameError::Header(m) if m.contains("isn't a header"))
        );
        assert!(
            matches!(refused(b"Content-Type: x\r\n\r\n"), FrameError::Header(m) if m.contains("no Content-Length"))
        );
        assert!(
            matches!(refused(b"Content-Length: -1\r\n\r\n"), FrameError::Header(m) if m.contains("isn't a number"))
        );
        assert!(matches!(
            refused(b"Content-Length:\r\n\r\n"),
            FrameError::Header(_)
        ));
        assert!(matches!(
            refused(b"Content-Length: 1\r\nContent-Length: 1\r\n\r\nx"),
            FrameError::Header(m) if m.contains("twice")
        ));
        assert!(
            matches!(refused(b"Content-Length: \xff\r\n\r\n"), FrameError::Header(m) if m.contains("text"))
        );
        assert!(matches!(
            refused(b"Content-Length: 2\n\n{}"),
            FrameError::Header(m) if m.contains("must end in")
        ));
        // A header block over the limit, whether or not it ends later, and
        // however the bytes are split.
        let block = |size: usize| {
            let mut block = b"Content-Length: 1\r\nX: ".to_vec();
            block.resize(size, b'x');
            [block, b"\r\n\r\nx".to_vec()].concat()
        };
        let long = block(MAX_HEADERS + 1);
        assert_eq!(refused(&long), FrameError::HeadersTooLong);
        let mut decoder = Decoder::default();
        assert!(decoder.push(&long[..MAX_HEADERS]).unwrap().is_empty());
        assert_eq!(
            decoder.push(&long[MAX_HEADERS..]),
            Err(FrameError::HeadersTooLong)
        );
        let just = block(MAX_HEADERS);
        assert_eq!(Decoder::default().push(&just).unwrap(), [b"x".to_vec()]);
        let mut decoder = Decoder::default();
        assert!(decoder.push(&just[..MAX_HEADERS + 2]).unwrap().is_empty());
        assert_eq!(
            decoder.push(&just[MAX_HEADERS + 2..]).unwrap(),
            [b"x".to_vec()]
        );

        // A message and garbage in one read: the message still counts, and
        // the break shows next (found by the garbage-output module test).
        let mut decoder = Decoder::default();
        let both = [framed("{}"), b"garbage\r\n\r\n".to_vec()].concat();
        assert_eq!(decoder.push(&both).unwrap(), [b"{}".to_vec()]);
        assert!(matches!(decoder.error(), Some(FrameError::Header(_))));
        assert!(decoder.push(b"").is_err());
        #[cfg(feature = "peer")]
        {
            let (inbound, mut received) = tokio::sync::mpsc::channel(4);
            std::thread::spawn(move || read_messages_blocking(std::io::Cursor::new(both), inbound));
            assert_eq!(received.blocking_recv(), Some(Ok(b"{}".to_vec())));
            assert!(matches!(received.blocking_recv(), Some(Err(_))));
        }

        // Cut off in the header or in the body.
        let mut decoder = Decoder::default();
        assert!(decoder.push(b"Content-Len").unwrap().is_empty());
        assert_eq!(decoder.finish(), Err(FrameError::Truncated));
        let mut decoder = Decoder::default();
        assert!(
            decoder
                .push(b"Content-Length: 4\r\n\r\n{}")
                .unwrap()
                .is_empty()
        );
        assert_eq!(decoder.finish(), Err(FrameError::Truncated));
        assert_eq!(decoder.push(b"}}").unwrap(), [b"{}}}".to_vec()]);
        decoder.finish().unwrap();
    }

    // covers: CORE-02
    // suite: unit
    #[cfg(feature = "peer")]
    #[tokio::test]
    async fn messages_cross_a_byte_stream_framed() {
        use tokio::sync::mpsc;
        let (near, far) = tokio::io::duplex(64);
        let (far_read, far_write) = tokio::io::split(far);
        let (near_read, near_write) = tokio::io::split(near);
        let (out, outbound) = mpsc::channel(4);
        let (inbound, mut received) = mpsc::channel(4);
        tokio::spawn(write_messages(near_write, outbound));
        tokio::spawn(read_messages(far_read, inbound));
        out.send(b"{\"a\":1}".to_vec()).await.unwrap();
        out.send(vec![b'x'; 1000]).await.unwrap();
        assert_eq!(received.recv().await, Some(Ok(b"{\"a\":1}".to_vec())));
        assert_eq!(received.recv().await, Some(Ok(vec![b'x'; 1000])));
        // Closing the channel closes the stream: a clean end.
        drop(out);
        assert_eq!(received.recv().await, None);
        drop((far_write, near_read));

        // Garbage, a stream cut short or a message over the limit arrives
        // as the reason it broke.
        let oversize = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE + 1);
        for bytes in [
            &b"hello\r\n\r\n"[..],
            b"Content-Length: 9\r\n\r\n{}",
            oversize.as_bytes(),
        ] {
            let (inbound, mut received) = mpsc::channel(4);
            tokio::spawn(read_messages(std::io::Cursor::new(bytes.to_vec()), inbound));
            let Some(Err(reason)) = received.recv().await else {
                panic!("a break");
            };
            assert!(
                ["header", "ended", "limit"]
                    .iter()
                    .any(|w| reason.contains(w)),
                "{reason}"
            );
            assert_eq!(received.recv().await, None);
        }

        // The blocking reader does the same, on its own thread.
        let (inbound, mut received) = mpsc::channel(4);
        let mut stream = encode_body(b"{}").unwrap();
        stream.extend_from_slice(b"Content-Length: 5\r\n\r\n{");
        std::thread::spawn(move || read_messages_blocking(std::io::Cursor::new(stream), inbound));
        assert_eq!(received.recv().await, Some(Ok(b"{}".to_vec())));
        assert!(matches!(received.recv().await, Some(Err(r)) if r.contains("ended")));
        assert_eq!(received.recv().await, None);
        let (inbound, mut received) = mpsc::channel(4);
        std::thread::spawn(move || {
            read_messages_blocking(std::io::Cursor::new(b"x\r\n\r\n"), inbound)
        });
        assert!(matches!(received.recv().await, Some(Err(r)) if r.contains("header")));

        // A read that fails is a break too.
        struct Failing;
        impl tokio::io::AsyncRead for Failing {
            fn poll_read(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
                _: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Err(std::io::Error::other("gone")))
            }
        }
        let (inbound, mut received) = mpsc::channel(4);
        tokio::spawn(read_messages(Failing, inbound));
        assert_eq!(
            received.recv().await,
            Some(Err("reading failed: gone".into()))
        );

        // The async reader also hands on a whole message before a break.
        let (inbound, mut received) = mpsc::channel(4);
        let both = [encode_body(b"{}").unwrap(), b"garbage\r\n\r\n".to_vec()].concat();
        tokio::spawn(read_messages(std::io::Cursor::new(both), inbound));
        assert_eq!(received.recv().await, Some(Ok(b"{}".to_vec())));
        assert!(matches!(received.recv().await, Some(Err(r)) if r.contains("header")));

        // Writing to a closed stream says why it stopped; so does a message
        // too big to frame.
        let (near, far) = tokio::io::duplex(64);
        drop(far);
        let (out, outbound) = mpsc::channel(1);
        out.send(b"{}".to_vec()).await.unwrap();
        assert!(write_messages(near, outbound).await.is_err());
        let (out, outbound) = mpsc::channel(1);
        out.send(vec![b' '; MAX_MESSAGE + 1]).await.unwrap();
        assert!(write_messages(tokio::io::sink(), outbound).await.is_err());
    }
}
