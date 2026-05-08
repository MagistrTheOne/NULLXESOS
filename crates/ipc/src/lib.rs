//! NULLXES Control IPC v1
//!
//! Wire format: line-delimited JSON. Each line is a complete `Envelope`.
//! Transport: Unix domain SOCK_STREAM at `$XDG_RUNTIME_DIR/nullxes/frame.sock`,
//! mode 0600, owned by the FRAME process.
//!
//! Versioning:
//!   - The `v` field is mandatory on every envelope and is currently `1`.
//!   - Servers reject unknown versions with `Response::version_mismatch`.
//!   - Adding new request kinds is a non-breaking change.
//!   - Removing or repurposing a kind is breaking and bumps `v`.
//!
//! Lifecycle:
//!   - FRAME creates and binds the socket on startup, drops file on shutdown.
//!   - Clients reconnect with exponential backoff: 100, 200, 400, … capped at 5000ms.
//!     After 32 attempts a fatal error is surfaced.
//!   - Server applies bounded-queue backpressure. Stale `Reload.Config` requests
//!     are coalesced (only the latest is honoured) to avoid head-of-line stalls.

pub mod envelope;
pub mod request;
pub mod response;
pub mod path;

// Convenient re-exports.
pub use path::{launcher_lock_path, socket_path, wayland_display_path};

#[cfg(feature = "client")]
pub mod client;

pub use envelope::{Envelope, Direction, PROTOCOL_VERSION};
pub use request::Request;
pub use response::{Response, ResponseData, IpcError};
pub use path::socket_path;
