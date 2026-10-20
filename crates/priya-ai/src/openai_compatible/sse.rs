//! Server-sent events, as streaming chat APIs send them: lines of
//! `data: …`, an event ending at a blank line, `:` comments for keep-alive.
//! Only `data` matters to these APIs; `event`, `id` and `retry` are read
//! and ignored, as the SSE spec allows. The llama.cpp server also reports
//! errors mid-stream on a non-standard `error:` line, which is kept.
//!
//! Bytes arrive in arbitrary pieces, so the parser keeps the unfinished
//! line between pushes. A lone `\r` as a line end and a leading byte-order
//! mark are allowed by the spec but sent by no model server we know of, so
//! they aren't handled.

/// The longest line, and the most data in one event, accepted. A model's chunk is a few hundred bytes; a
/// server that sends megabytes without a newline is broken, and buffering
/// it would let it fill the daemon's memory.
pub const MAX_LINE: usize = 1024 * 1024;

/// The data of one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseEvent {
    Data(String),
    /// `data: [DONE]`: OpenAI-style streams end with it.
    Done,
    /// An `error:` line (llama.cpp), with what followed the colon.
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SseError {
    #[error("a streaming line or event is longer than {MAX_LINE} bytes")]
    LineTooLong,
    #[error("a streaming line isn't UTF-8")]
    NotUtf8,
}

#[derive(Debug, Default)]
pub struct SseParser {
    line: Vec<u8>,
    data: Option<String>,
    /// The event's data has a line already (which may be empty).
    continued: bool,
}

impl SseParser {
    /// Reads `bytes` and returns the events they complete.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, SseError> {
        let mut events = Vec::new();
        for &byte in bytes {
            if byte == b'\n' {
                let line = std::mem::take(&mut self.line);
                self.line(&line, &mut events)?;
            } else {
                if self.line.len() >= MAX_LINE {
                    return Err(SseError::LineTooLong);
                }
                self.line.push(byte);
            }
        }
        Ok(events)
    }

    /// The event left when the connection closed without a blank line.
    pub fn finish(&mut self) -> Result<Option<SseEvent>, SseError> {
        let mut events = Vec::new();
        let line = std::mem::take(&mut self.line);
        if !line.is_empty() {
            self.line(&line, &mut events)?;
        }
        self.dispatch(&mut events);
        Ok(events.pop())
    }

    fn line(&mut self, line: &[u8], events: &mut Vec<SseEvent>) -> Result<(), SseError> {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            self.dispatch(events);
            return Ok(());
        }
        let line = std::str::from_utf8(line).map_err(|_| SseError::NotUtf8)?;
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        // One optional space after the colon belongs to the syntax.
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "data" => {
                let data = self.data.get_or_insert_with(String::new);
                // Many data lines without a blank one mustn't add up to
                // more than one long line could.
                if data.len() + value.len() >= MAX_LINE {
                    return Err(SseError::LineTooLong);
                }
                if !data.is_empty() || self.continued {
                    data.push('\n');
                }
                data.push_str(value);
                self.continued = true;
            }
            "error" => events.push(SseEvent::Error(value.to_owned())),
            _ => {}
        }
        // A comment (empty field) or another field: nothing to keep.
        Ok(())
    }

    fn dispatch(&mut self, events: &mut Vec<SseEvent>) {
        self.continued = false;
        if let Some(data) = self.data.take() {
            events.push(if data.trim() == "[DONE]" {
                SseEvent::Done
            } else {
                SseEvent::Data(data)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(text: &str) -> SseEvent {
        SseEvent::Data(text.to_owned())
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn sse_events_split_anywhere_still_parse() {
        let stream = b": keep-alive\r\ndata: {\"a\":1}\r\n\r\nevent: x\ndata:{\"b\":2}\nid: 7\n\ndata: [DONE]\n\n";
        let expected = vec![data("{\"a\":1}"), data("{\"b\":2}"), SseEvent::Done];
        // Every split point gives the same events.
        for split in 0..stream.len() {
            let mut parser = SseParser::default();
            let mut events = parser.push(&stream[..split]).unwrap();
            events.extend(parser.push(&stream[split..]).unwrap());
            assert_eq!(events, expected, "split at {split}");
            assert_eq!(parser.finish().unwrap(), None);
        }
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn multi_line_data_joins_and_an_unfinished_event_is_kept_for_the_end() {
        let mut parser = SseParser::default();
        let events = parser.push(b"data: one\ndata:  two\n\ndata: last").unwrap();
        assert_eq!(events, vec![data("one\n two")]);
        assert_eq!(parser.finish().unwrap(), Some(data("last")));
        // Blank lines without data send nothing.
        assert_eq!(parser.push(b"\n\n\n").unwrap(), vec![]);
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn oversized_or_invalid_lines_are_errors() {
        let mut parser = SseParser::default();
        let long = vec![b'a'; MAX_LINE + 1];
        assert_eq!(parser.push(&long), Err(SseError::LineTooLong));
        let mut parser = SseParser::default();
        assert_eq!(parser.push(b"data: \xff\xfe\n"), Err(SseError::NotUtf8));

        // Many short data lines in one event count together.
        let mut parser = SseParser::default();
        let line = format!("data: {}\n", "a".repeat(1000));
        let many = line.repeat(MAX_LINE / 1000 + 1);
        assert_eq!(parser.push(many.as_bytes()), Err(SseError::LineTooLong));
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn llama_cpp_error_lines_are_kept() {
        let mut parser = SseParser::default();
        let events = parser
            .push(b"data: {\"a\":1}\n\nerror: {\"code\":500,\"message\":\"out of memory\"}\n\n")
            .unwrap();
        assert_eq!(
            events,
            vec![
                data("{\"a\":1}"),
                SseEvent::Error("{\"code\":500,\"message\":\"out of memory\"}".into())
            ]
        );
        // An empty data line still counts as a line of the event.
        let mut parser = SseParser::default();
        assert_eq!(
            parser.push(b"data:\ndata: x\n\n").unwrap(),
            vec![data("\nx")]
        );
    }
}
