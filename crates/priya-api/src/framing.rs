//! Framing on the surfaces' socket (02-core §4): one JSON value per line
//! (newline-delimited JSON), at most [`MAX_LINE`] bytes. This is what
//! Quickshell's `Socket` with a `SplitParser` reads. Compact JSON never
//! contains a raw newline, so a line is always one whole message.
//!
//! Modules talk over stdio with `Content-Length` framing instead (02 §5.2).

use serde::Serialize;

/// The longest line accepted: 16 MiB.
pub const MAX_LINE: usize = 16 * 1024 * 1024;

/// What [`LineSplitter::push`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// One whole line, without its line ending.
    Complete(Vec<u8>),
    /// A line longer than the limit; it was dropped up to its end.
    TooLong,
}

/// Splits a byte stream into lines, however the bytes arrive.
#[derive(Debug)]
pub struct LineSplitter {
    buffer: Vec<u8>,
    max: usize,
    /// Inside a line that was too long, until its newline.
    skipping: bool,
}

impl Default for LineSplitter {
    fn default() -> Self {
        Self::new(MAX_LINE)
    }
}

impl LineSplitter {
    pub fn new(max: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max,
            skipping: false,
        }
    }

    /// Adds `bytes` and returns the lines they finish. Blank lines are
    /// skipped; a too-long line is reported once.
    pub fn push(&mut self, mut bytes: &[u8]) -> Vec<Line> {
        let mut lines = Vec::new();
        while !bytes.is_empty() {
            match bytes.iter().position(|&b| b == b'\n') {
                Some(end) => {
                    let (head, rest) = bytes.split_at(end);
                    bytes = &rest[1..];
                    if self.skipping {
                        self.skipping = false;
                        continue;
                    }
                    // One byte over the limit may be the `\r` of a CRLF
                    // ending, which may have arrived in an earlier read.
                    if self.buffer.len() + head.len() > self.max + 1 {
                        self.buffer.clear();
                        lines.push(Line::TooLong);
                        continue;
                    }
                    self.buffer.extend_from_slice(head);
                    if self.buffer.last() == Some(&b'\r') {
                        self.buffer.pop();
                    }
                    if self.buffer.len() > self.max {
                        self.buffer.clear();
                        lines.push(Line::TooLong);
                        continue;
                    }
                    lines.extend(self.take());
                }
                None => {
                    if !self.skipping {
                        // One byte over may be the `\r` of a CRLF ending;
                        // the newline decides.
                        if self.buffer.len() + bytes.len() > self.max + 1 {
                            self.buffer.clear();
                            self.skipping = true;
                            lines.push(Line::TooLong);
                        } else {
                            self.buffer.extend_from_slice(bytes);
                        }
                    }
                    bytes = &[];
                }
            }
        }
        lines
    }
}

impl LineSplitter {
    /// The last line when the stream ends without a newline (a client that
    /// half-closes, `printf … | socat`); call it at end of input.
    pub fn finish(&mut self) -> Option<Line> {
        if std::mem::take(&mut self.skipping) {
            self.buffer.clear();
            return None;
        }
        let line = std::mem::take(&mut self.buffer);
        let line = line.strip_suffix(b"\r").map(<[u8]>::to_vec).unwrap_or(line);
        if line.len() > self.max {
            return Some(Line::TooLong);
        }
        (!line.iter().all(u8::is_ascii_whitespace)).then_some(Line::Complete(line))
    }

    fn take(&mut self) -> Option<Line> {
        let line = std::mem::take(&mut self.buffer);
        (!line.iter().all(u8::is_ascii_whitespace)).then_some(Line::Complete(line))
    }
}

/// Why a message couldn't be framed.
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("the message is {0} bytes, more than the {MAX_LINE}-byte limit")]
    TooLong(usize),
}

/// `value` as one line of compact JSON, ending in a newline, if it fits in
/// [`MAX_LINE`].
pub fn encode(value: &impl Serialize) -> Result<Vec<u8>, EncodeError> {
    let mut bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_LINE {
        return Err(EncodeError::TooLong(bytes.len()));
    }
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete(text: &str) -> Line {
        Line::Complete(text.as_bytes().to_vec())
    }

    // covers: CORE-04
    // suite: unit
    // acceptance: 0.7
    // regression: 0.7
    #[test]
    fn ndjson_framing_handles_partial_reads_and_16mib_limit() {
        let mut splitter = LineSplitter::default();
        // A message split across reads, two in one read, CRLF, blank lines.
        assert_eq!(splitter.push(b"{\"a\":"), []);
        assert_eq!(
            splitter.push(b"1}\n{\"b\":2}\r\n\n  \n{\"c\""),
            [complete("{\"a\":1}"), complete("{\"b\":2}")]
        );
        assert_eq!(splitter.push(b":3}\n"), [complete("{\"c\":3}")]);

        // Exactly the limit is fine; one byte more is refused once, and the
        // stream carries on with the next line.
        let mut splitter = LineSplitter::new(8);
        assert_eq!(splitter.push(b"12345678\n"), [complete("12345678")]);
        assert_eq!(splitter.push(b"1234"), []);
        // Nine bytes might still end in a CRLF; more data settles it.
        assert_eq!(splitter.push(b"56789"), []);
        assert_eq!(splitter.push(b"more and more"), [Line::TooLong]);
        assert_eq!(splitter.push(b"\nok\n"), [complete("ok")]);
        assert_eq!(
            splitter.push(b"123456789\nnext\n"),
            [Line::TooLong, complete("next")]
        );

        // The real limit is 16 MiB.
        assert_eq!(MAX_LINE, 16 * 1024 * 1024);
        let mut splitter = LineSplitter::default();
        let big = vec![b'x'; MAX_LINE];
        assert_eq!(splitter.push(&big), []);
        assert_eq!(
            splitter.push(b"\r\n"),
            [Line::Complete(big.clone())],
            "CRLF doesn't count"
        );
        assert_eq!(splitter.push(&big), []);
        assert_eq!(splitter.push(b"y\n"), [Line::TooLong]);

        assert_eq!(
            encode(&serde_json::json!({"a": "x\ny"})).unwrap(),
            b"{\"a\":\"x\\ny\"}\n"
        );
        let too_big = "x".repeat(MAX_LINE);
        assert!(matches!(encode(&too_big), Err(EncodeError::TooLong(_))));
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn the_last_line_counts_without_a_newline() {
        let mut splitter = LineSplitter::new(8);
        assert_eq!(splitter.push(b"one\ntwo"), [complete("one")]);
        assert_eq!(splitter.finish(), Some(complete("two")));
        assert_eq!(splitter.finish(), None);
        assert_eq!(splitter.push(b"  \r"), []);
        assert_eq!(splitter.finish(), None, "blank");
        assert_eq!(splitter.push(b"123456789"), []);
        assert_eq!(
            splitter.finish(),
            Some(Line::TooLong),
            "too long at the end"
        );
        assert_eq!(splitter.push(b"1234567890"), [Line::TooLong]);
        assert_eq!(splitter.finish(), None, "already reported");
        assert_eq!(splitter.push(b"12345678\r\n"), [complete("12345678")]);

        // A CRLF split across reads, wherever the boundary falls.
        assert_eq!(splitter.push(b"{}\r"), []);
        assert_eq!(splitter.push(b"\n"), [complete("{}")]);
        assert_eq!(splitter.push(b"12345678\r"), []);
        assert_eq!(splitter.push(b"\n"), [complete("12345678")]);
        assert_eq!(splitter.push(b"12345678"), []);
        assert_eq!(splitter.push(b"\r"), []);
        assert_eq!(splitter.push(b"\n"), [complete("12345678")]);
        // Nine bytes that aren't a CRLF are still too long.
        assert_eq!(splitter.push(b"123456789"), []);
        assert_eq!(splitter.push(b"\n"), [Line::TooLong]);
    }
}
