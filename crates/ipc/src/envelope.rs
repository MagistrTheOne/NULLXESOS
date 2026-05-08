//! Envelope shared by all IPC messages.

use serde::{Deserialize, Serialize};

use crate::request::Request;
use crate::response::Response;

/// Current wire protocol version.
pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Wire version. Always `PROTOCOL_VERSION` for v1 senders.
    pub v: u8,
    /// Monotonic correlation id chosen by the request sender; servers echo it
    /// in the corresponding response.
    pub id: u64,
    #[serde(flatten)]
    pub direction: Direction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Direction {
    Request(Request),
    Response(Response),
}

impl Envelope {
    pub fn request(id: u64, req: Request) -> Self {
        Self { v: PROTOCOL_VERSION, id, direction: Direction::Request(req) }
    }
    pub fn response(id: u64, resp: Response) -> Self {
        Self { v: PROTOCOL_VERSION, id, direction: Direction::Response(resp) }
    }
}
