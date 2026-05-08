//! Server-side `ext-session-lock-v1`.
//!
//! A lock client (we ship `nullxes-lock`) holds the screen by:
//!   1. Binding `ext_session_lock_manager_v1` and calling `lock`.
//!   2. Receiving `locked` (we accept; only one lock client at a time).
//!   3. Creating a `lock_surface` per output and attaching its first buffer.
//!   4. Sending `unlock_and_destroy` after PAM authentication succeeds.
//!
//! While locked, FRAME redirects all input focus to the lock surface and
//! refuses to send pointer/keyboard events to other clients.

use smithay::{
    delegate_session_lock,
    output::Output,
    wayland::session_lock::{
        LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker,
    },
};

use crate::state::{LockSession, NullxesState};

impl SessionLockHandler for NullxesState {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_state
    }

    fn lock(&mut self, locker: SessionLocker) {
        if self.lock.is_some() {
            tracing::warn!("session-lock requested while already locked; rejecting new lock");
            // smithay drops the locker on Drop without sending `locked`; the
            // protocol dictates this is interpreted as the manager refusing.
            drop(locker);
            return;
        }
        tracing::info!("session locked");
        self.lock = Some(LockSession {
            locker,
            surfaces: Vec::new(),
        });
        // Clear keyboard focus from windows; lock surfaces will receive focus
        // once their first buffer commits.
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        if let Some(kbd) = self.seat.get_keyboard() {
            kbd.set_focus(self, None, serial);
        }
    }

    fn unlock(&mut self) {
        tracing::info!("session unlocked");
        self.lock = None;
        // Restore keyboard focus to the topmost window of the active workspace.
        if let Some(top) = self
            .active_space()
            .elements()
            .next_back()
            .and_then(|w| w.wl_surface())
            .map(|s| s.into_owned())
        {
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            if let Some(kbd) = self.seat.get_keyboard() {
                kbd.set_focus(self, Some(&top), serial);
            }
        }
    }

    fn new_surface(&mut self, surface: LockSurface, output: smithay::reexports::wayland_server::protocol::wl_output::WlOutput) {
        let Some(lock) = self.lock.as_mut() else {
            tracing::warn!("lock surface arrived without active lock; closing");
            return;
        };
        // Configure to output size.
        if let Some(o) = Output::from_resource(&output) {
            if let Some(mode) = o.current_mode() {
                surface.send_configure(mode.size, smithay::utils::SERIAL_COUNTER.next_serial());
            }
        }
        lock.surfaces.push(surface);
    }
}

delegate_session_lock!(NullxesState);
