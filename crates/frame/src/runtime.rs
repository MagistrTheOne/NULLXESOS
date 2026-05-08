//! Tokio runtime holder — owned for the FRAME process lifetime.
//!
//! FRAME drives the wayland event loop on the calloop main thread (not async),
//! but D-Bus listeners (login1, UPower, NM), the Control IPC server, and the
//! PipeWire mainloop all want async. We host one multi-thread tokio runtime
//! and hand callers a `Handle` to spawn onto it.

use std::sync::OnceLock;

use anyhow::Result;
use tokio::runtime::{Handle, Runtime as TokioRuntime};

static GLOBAL: OnceLock<RuntimeHolder> = OnceLock::new();

struct RuntimeHolder {
    handle: Handle,
    // Keep the runtime alive for the process lifetime.
    _rt: &'static TokioRuntime,
}

pub struct Runtime;

impl Runtime {
    pub fn install_global() -> Result<()> {
        if GLOBAL.get().is_some() {
            return Ok(());
        }
        let rt = TokioRuntime::new()?;
        // Leak the runtime so tasks live as long as the process. FRAME never
        // actually rebuilds the runtime, so this is one allocation per process.
        let rt_static: &'static TokioRuntime = Box::leak(Box::new(rt));
        let handle = rt_static.handle().clone();
        let _ = GLOBAL.set(RuntimeHolder { handle, _rt: rt_static });
        Ok(())
    }

    /// Returns the global tokio handle. Panics in debug builds if the runtime
    /// was not installed; returns a fresh `Handle::current()` in release if
    /// called from inside a runtime context.
    pub fn handle() -> Handle {
        if let Some(g) = GLOBAL.get() {
            return g.handle.clone();
        }
        debug_assert!(false, "Runtime::handle called before install_global");
        Handle::current()
    }
}
