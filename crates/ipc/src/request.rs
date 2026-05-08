//! Request kinds the FRAME compositor accepts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum Request {
    /// Switch the active workspace on the focused output.
    #[serde(rename = "Switch.Workspace")]
    SwitchWorkspace { idx: usize },

    /// Move the focused window to the given workspace index.
    #[serde(rename = "Move.Focused")]
    MoveFocused { target: usize },

    /// Re-read configuration files from disk and apply (idempotent).
    #[serde(rename = "Reload.Config")]
    ReloadConfig,

    /// Request the current compositor state snapshot.
    #[serde(rename = "Get.State")]
    GetState,

    /// Spawn the system launcher overlay (idempotent — held by lockfile).
    #[serde(rename = "Spawn.Launcher")]
    SpawnLauncher,

    /// Trigger session lock (spawns `nullxes-lock`).
    #[serde(rename = "Session.Lock")]
    SessionLock,

    /// Quit the compositor cleanly. Used by debug tooling and shutdown sequences.
    #[serde(rename = "Compositor.Quit")]
    Quit,
}
