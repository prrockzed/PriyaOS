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
    #[error("a message of {0} bytes is over the {MAX_MESSAGE}-byte limit")]
    TooLong(usize),
    #[error("the message header is over {MAX_HEADERS} bytes")]
    HeadersTooLong,
    /// The stream ended inside a message.
    #[error("the stream ended in the middle of a message")]
    Truncated,
}

/// Turns a byte stream into message bodies, however the bytes arrive.
#[derive(Debug)]
pub struct Decoder {
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
    pub fn new(max: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max,
            body: None,
            failed: None,
        }
    }

    /// Adds `bytes` and returns the bodies they complete, or the error that
    /// broke the stream (again on every later call).
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        self.buffer.extend_from_slice(bytes);
        let mut bodies = Vec::new();
        loop {
            match self.body {
                Some(length) if self.buffer.len() >= length => {
                    let rest = self.buffer.split_off(length);
                    bodies.push(std::mem::replace(&mut self.buffer, rest));
                    self.body = None;
                }
                Some(_) => break,
                None => match self.header() {
                    Ok(Some(length)) => self.body = Some(length),
                    Ok(None) => break,
                    Err(error) => {
                        self.buffer = Vec::new();
                        self.failed = Some(error.clone());
                        return Err(error);
                    }
                },
            }
        }
        Ok(bodies)
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

    /// Takes a complete header block off the buffer and returns its
    /// `Content-Length`; none while the block is incomplete.
    fn header(&mut self) -> Result<Option<usize>, FrameError> {
        let Some(end) = self
            .buffer
            .windows(END_OF_HEADERS.len())
            .position(|w| w == END_OF_HEADERS)
        else {
            return if self.buffer.len() > MAX_HEADERS {
                Err(FrameError::HeadersTooLong)
            } else {
                Ok(None)
            };
        };
        if end > MAX_HEADERS {
            return Err(FrameError::HeadersTooLong);
        }
        let block = std::str::from_utf8(&self.buffer[..end])
            .map_err(|_| FrameError::Header("it isn't text".into()))?;
        let length = content_length(block)?;
        if length > self.max {
            return Err(FrameError::TooLong(length));
        }
        self.buffer.drain(..end + END_OF_HEADERS.len());
        Ok(Some(length))
    }
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
        assert_eq!(
            decoder.push(b"Content-Length: 5\r\n\r\n"),
            Err(FrameError::TooLong(5))
        );
        assert_eq!(decoder.push(&framed("ok")), Err(FrameError::TooLong(5)));
        assert_eq!(decoder.finish(), Err(FrameError::TooLong(5)));
        let mut decoder = Decoder::default();
        let huge = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE + 1);
        assert_eq!(
            decoder.push(huge.as_bytes()),
            Err(FrameError::TooLong(MAX_MESSAGE + 1))
        );
        let mut decoder = Decoder::default();
        assert_eq!(
            decoder.push(b"Content-Length: 99999999999999999999999\r\n\r\n"),
            Err(FrameError::TooLong(usize::MAX))
        );

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
        // A header block that never ends, whether or not it ends later.
        assert_eq!(
            refused(&vec![b'x'; MAX_HEADERS + 1]),
            FrameError::HeadersTooLong
        );
        let long = [vec![b'x'; MAX_HEADERS + 1], b": 1\r\n\r\n".to_vec()].concat();
        assert_eq!(refused(&long), FrameError::HeadersTooLong);

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
}
