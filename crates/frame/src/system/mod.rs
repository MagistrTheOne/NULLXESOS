//! System-service listeners that feed the in-process panel modules.
//!
//! Each listener runs as a tokio task on the FRAME global runtime, owns its
//! own D-Bus / PipeWire connection, and updates `ModuleSnapshot` under a
//! lock. The render path reads the snapshot once per frame.
//!
//! Listeners are spawned at compositor startup and cancelled on shutdown by
//! dropping the `tokio::sync::watch::Sender<bool>` shutdown signal.

pub mod login1;
pub mod nm;
pub mod upower;

use std::sync::Arc;

use parking_lot::Mutex;

use crate::panel::ModuleSnapshot;

#[derive(Clone)]
pub struct ListenersHandle {
    pub modules: Arc<Mutex<ModuleSnapshot>>,
}

impl ListenersHandle {
    pub fn new(modules: Arc<Mutex<ModuleSnapshot>>) -> Self {
        Self { modules }
    }

    pub fn spawn_all(&self, runtime: &tokio::runtime::Handle) {
        let me = self.clone();
        runtime.spawn(async move {
            if let Err(e) = upower::run(me.modules.clone()).await {
                tracing::warn!(?e, "upower listener exited");
            }
        });
        let me2 = self.clone();
        runtime.spawn(async move {
            if let Err(e) = nm::run(me2.modules.clone()).await {
                tracing::warn!(?e, "networkmanager listener exited");
            }
        });
        // login1 listener handles suspend/resume — wired separately in Phase 2.
    }
}
