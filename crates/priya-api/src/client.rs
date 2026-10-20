//! [`Client`]: talks to priyad over its socket, one request at a time.
//!
//! Notifications that arrive while waiting for a response are kept, in
//! order, for [`Client::next_notification`].

use std::collections::VecDeque;
use std::io;
use std::path::Path;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use crate::error::ApiError;
use crate::framing::{EncodeError, Line, LineSplitter, encode};
use crate::methods::Method;
use crate::rpc::{Id, Outcome, Request, Response};

/// Why a request didn't get an answer.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// No daemon is listening on the socket.
    #[error("priyad isn't running ({0})")]
    Unreachable(io::Error),
    #[error("lost the connection to priyad: {0}")]
    Io(#[from] io::Error),
    /// The daemon sent something that isn't a valid answer.
    #[error("priyad's answer couldn't be read: {0}")]
    Protocol(String),
    /// The daemon answered with an error.
    #[error("{0}")]
    Api(ApiError),
}

impl From<EncodeError> for ClientError {
    fn from(e: EncodeError) -> Self {
        Self::Protocol(e.to_string())
    }
}

/// A connection to priyad.
pub struct Client {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    splitter: LineSplitter,
    lines: VecDeque<Vec<u8>>,
    notifications: VecDeque<Request>,
    next_id: i64,
    buffer: Vec<u8>,
}

impl Client {
    /// Connects to the socket at `path`.
    pub async fn connect(path: &Path) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(path)
            .await
            .map_err(|e| match e.kind() {
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => {
                    ClientError::Unreachable(e)
                }
                _ => ClientError::Io(e),
            })?;
        let (reader, writer) = stream.into_split();
        Ok(Self {
            reader,
            writer,
            splitter: LineSplitter::default(),
            lines: VecDeque::new(),
            notifications: VecDeque::new(),
            next_id: 0,
            buffer: vec![0_u8; 64 * 1024],
        })
    }

    /// Calls method `M` and waits for its result.
    pub async fn call<M: Method>(&mut self, params: &M::Params) -> Result<M::Result, ClientError> {
        let value = self
            .call_raw(
                M::NAME,
                serde_json::to_value(params).map_err(|e| ClientError::Protocol(e.to_string()))?,
            )
            .await?;
        serde_json::from_value(value).map_err(|e| ClientError::Protocol(e.to_string()))
    }

    /// Calls `method` with JSON params and returns the JSON result.
    pub async fn call_raw(&mut self, method: &str, params: Value) -> Result<Value, ClientError> {
        self.next_id += 1;
        let id = Id::from(self.next_id);
        let request = Request::call(id.clone(), method, params);
        self.writer.write_all(&encode(&request)?).await?;
        loop {
            let line = self.next_line().await?;
            let value: Value =
                serde_json::from_slice(&line).map_err(|e| ClientError::Protocol(e.to_string()))?;
            if value.get("method").is_some() {
                let notification: Request = serde_json::from_value(value)
                    .map_err(|e| ClientError::Protocol(e.to_string()))?;
                self.notifications.push_back(notification);
                continue;
            }
            let response: Response =
                serde_json::from_value(value).map_err(|e| ClientError::Protocol(e.to_string()))?;
            if response.id != id {
                return Err(ClientError::Protocol(format!(
                    "an answer to {:?} while waiting for {id:?}",
                    response.id
                )));
            }
            return match response.outcome {
                Outcome::Result(result) => Ok(result),
                Outcome::Error(error) => Err(ClientError::Api(error.data)),
            };
        }
    }

    /// The next notification (method and params); none once the daemon
    /// closes the connection.
    pub async fn next_notification(&mut self) -> Result<Option<Request>, ClientError> {
        if let Some(notification) = self.notifications.pop_front() {
            return Ok(Some(notification));
        }
        loop {
            let line = match self.next_line().await {
                Ok(line) => line,
                Err(ClientError::Io(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(e),
            };
            let value: Value =
                serde_json::from_slice(&line).map_err(|e| ClientError::Protocol(e.to_string()))?;
            if value.get("method").is_some() {
                let notification = serde_json::from_value(value)
                    .map_err(|e| ClientError::Protocol(e.to_string()))?;
                return Ok(Some(notification));
            }
            // A stray response has no one waiting for it.
        }
    }

    async fn next_line(&mut self) -> Result<Vec<u8>, ClientError> {
        loop {
            if let Some(line) = self.lines.pop_front() {
                return Ok(line);
            }
            let n = self.reader.read(&mut self.buffer).await?;
            if n == 0 {
                if let Some(Line::Complete(line)) = self.splitter.finish() {
                    return Ok(line);
                }
                return Err(ClientError::Io(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "priyad closed the connection",
                )));
            }
            for line in self.splitter.push(&self.buffer[..n]) {
                match line {
                    Line::Complete(line) => self.lines.push_back(line),
                    Line::TooLong => {
                        return Err(ClientError::Protocol("an answer longer than 16 MiB".into()));
                    }
                }
            }
        }
    }
}
