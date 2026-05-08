//! Response payloads returned by the FRAME compositor.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum Response {
    Ok { data: Option<ResponseData> },
    Err { error: IpcError },
}

impl Response {
    pub fn ok() -> Self { Self::Ok { data: None } }
    pub fn ok_with(data: ResponseData) -> Self { Self::Ok { data: Some(data) } }
    pub fn err(error: IpcError) -> Self { Self::Err { error } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum ResponseData {
    State(StateSnapshot),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub workspace_count:  usize,
    pub active_workspace: usize,
    pub occupied:         Vec<usize>,
    pub xwayland_ready:   bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "code", content = "message")]
pub enum IpcError {
    #[error("protocol version mismatch (server supports {supported:?})")]
    #[serde(rename = "version_mismatch")]
    VersionMismatch { supported: Vec<u8> },

    #[error("server is busy; try again later")]
    #[serde(rename = "busy")]
    Busy,

    #[error("invalid request: {0}")]
    #[serde(rename = "invalid_request")]
    InvalidRequest(String),

    #[error("unknown request kind")]
    #[serde(rename = "unknown_kind")]
    UnknownKind,

    #[error("internal compositor error: {0}")]
    #[serde(rename = "internal")]
    Internal(String),

    #[error("not implemented")]
    #[serde(rename = "not_implemented")]
    NotImplemented,
}
