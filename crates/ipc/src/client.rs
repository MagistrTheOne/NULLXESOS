//! Async client implementation for NULLXES Control IPC v1.
//!
//! Reconnection policy: exponential backoff (100, 200, 400, …) capped at 5000ms,
//! up to 32 attempts before surfacing a fatal error. Callers awaiting `request()`
//! see a `Busy` if the server is actively backpressured.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::envelope::{Direction, Envelope, PROTOCOL_VERSION};
use crate::path::socket_path;
use crate::request::Request;
use crate::response::{IpcError, Response};

const RECONNECT_INITIAL_MS: u64 = 100;
const RECONNECT_CAP_MS:     u64 = 5_000;
const RECONNECT_MAX_TRIES:  u32 = 32;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("server returned error: {0}")]
    Server(IpcError),
    #[error("server hung up before responding")]
    Hangup,
    #[error("max reconnect attempts ({0}) exhausted")]
    ReconnectExhausted(u32),
    #[error("unexpected message direction (got response while expecting another)")]
    Misframed,
}

pub struct Client {
    stream:  BufReader<UnixStream>,
    next_id: AtomicU64,
}

impl Client {
    pub async fn connect() -> Result<Self, ClientError> {
        let path = socket_path();
        let mut delay_ms = RECONNECT_INITIAL_MS;
        for attempt in 1..=RECONNECT_MAX_TRIES {
            match UnixStream::connect(&path).await {
                Ok(s) => {
                    return Ok(Self {
                        stream:  BufReader::new(s),
                        next_id: AtomicU64::new(1),
                    });
                }
                Err(e) => {
                    tracing::warn!(?e, attempt, ?path, "frame ipc connect failed");
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms = (delay_ms * 2).min(RECONNECT_CAP_MS);
                }
            }
        }
        Err(ClientError::ReconnectExhausted(RECONNECT_MAX_TRIES))
    }

    pub async fn request(&mut self, req: Request) -> Result<Response, ClientError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let env = Envelope { v: PROTOCOL_VERSION, id, direction: Direction::Request(req) };
        let mut bytes = serde_json::to_vec(&env)?;
        bytes.push(b'\n');
        self.stream.get_mut().write_all(&bytes).await?;
        self.stream.get_mut().flush().await?;

        let mut line = String::new();
        let n = self.stream.read_line(&mut line).await?;
        if n == 0 {
            return Err(ClientError::Hangup);
        }
        let resp: Envelope = serde_json::from_str(line.trim_end())?;
        match resp.direction {
            Direction::Response(r) => Ok(r),
            Direction::Request(_) => Err(ClientError::Misframed),
        }
    }
}
