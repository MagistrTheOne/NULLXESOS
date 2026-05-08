//! Shared error type for NULLXES Wayland clients.

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("wayland connection failed: {0}")]
    Connect(#[from] wayland_client::ConnectError),

    #[error("wayland dispatch failed: {0}")]
    Dispatch(#[from] wayland_client::DispatchError),

    #[error("wayland global bind failed: {0}")]
    Bind(#[from] wayland_client::globals::BindError),

    #[error("wayland global init failed: {0}")]
    GlobalsInit(#[from] wayland_client::globals::GlobalError),

    #[error("required wayland global missing: {name}")]
    GlobalMissing { name: &'static str },

    #[error("shm allocation failed: {0}")]
    Shm(String),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("xkbcommon error: {0}")]
    Xkb(String),

    #[error("buffer pool exhausted (all {count} buffers in flight)")]
    BufferPoolExhausted { count: usize },

    #[error("config error: {0}")]
    Config(String),
}

pub type Result<T, E = ClientError> = std::result::Result<T, E>;
