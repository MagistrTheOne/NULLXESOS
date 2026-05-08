//! Idle pipeline (replaces swayidle for NULLXES sessions).
//!
//! Tracks last activity timestamp on every input event; periodic timer evaluates
//! configured thresholds:
//!   - `dim_ms` → reduces output brightness (Phase 2 hookup; no-op until then).
//!   - `lock_ms` → spawns `nullxes-lock` (idempotent via launcher.lock-style file).
//!   - `suspend_ms` → calls `org.freedesktop.login1.Manager.Suspend` via D-Bus.
//!
//! This module also serves the `ext-idle-notify-v1` server so external clients
//! (e.g. third-party screensavers) can register inhibitors. The protocol
//! delegation is handled in `handlers/` once `smithay::wayland::idle_notify`
//! is wired in 0.2; for Stage 1 we own the timing locally.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::config::IdleConfig;

#[derive(Clone)]
pub struct IdleTracker {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    last_activity: Instant,
    cfg:           IdleConfig,
    dimmed:        bool,
    locked_pending:bool,
}

impl IdleTracker {
    pub fn new(cfg: IdleConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                last_activity: Instant::now(),
                cfg,
                dimmed: false,
                locked_pending: false,
            })),
        }
    }

    pub fn touch(&self) {
        let mut g = self.inner.lock();
        g.last_activity = Instant::now();
        g.dimmed = false;
        g.locked_pending = false;
    }

    pub fn update_config(&self, cfg: IdleConfig) {
        let mut g = self.inner.lock();
        g.cfg = cfg;
    }

    /// Called by the calloop timer source. Returns the next set of actions.
    pub fn tick(&self) -> IdleActions {
        let mut g = self.inner.lock();
        let elapsed = g.last_activity.elapsed();

        let mut actions = IdleActions::default();

        if g.cfg.dim_ms > 0 && elapsed >= Duration::from_millis(g.cfg.dim_ms) && !g.dimmed {
            g.dimmed = true;
            actions.dim = true;
        }

        if g.cfg.lock_ms > 0 && elapsed >= Duration::from_millis(g.cfg.lock_ms) && !g.locked_pending {
            g.locked_pending = true;
            actions.lock = true;
        }

        if g.cfg.suspend_ms > 0 && elapsed >= Duration::from_millis(g.cfg.suspend_ms) {
            actions.suspend = true;
        }

        actions
    }
}

#[derive(Default, Debug)]
pub struct IdleActions {
    pub dim:     bool,
    pub lock:    bool,
    pub suspend: bool,
}
