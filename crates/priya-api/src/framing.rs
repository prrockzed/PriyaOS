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
                    if self.buffer.len() + head.len() > self.max {
                        self.buffer.clear();
                        lines.push(Line::TooLong);
                        continue;
                    }
                    self.buffer.extend_from_slice(head);
                    let mut line = std::mem::take(&mut self.buffer);
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    if !line.iter().all(u8::is_ascii_whitespace) {
                        lines.push(Line::Complete(line));
                    }
                }
                None => {
                    if !self.skipping {
                        if self.buffer.len() + bytes.len() > self.max {
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

/// `value` as one line of compact JSON, ending in a newline.
pub fn encode(value: &impl Serialize) -> serde_json::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)?;
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
        assert_eq!(splitter.push(b"56789"), [Line::TooLong]);
        assert_eq!(splitter.push(b"more and more"), []);
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
        assert_eq!(splitter.push(b"\n").len(), 1);
        assert_eq!(splitter.push(&big), []);
        assert_eq!(splitter.push(b"y\n"), [Line::TooLong]);

        assert_eq!(
            encode(&serde_json::json!({"a": "x\ny"})).unwrap(),
            b"{\"a\":\"x\\ny\"}\n"
        );
    }
}
