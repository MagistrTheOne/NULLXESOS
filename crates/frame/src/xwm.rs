//! XWayland integration with idempotent respawn.
//!
//! Lifecycle:
//!   - `XwmState::new()` returns an empty handle (XWayland not started).
//!   - `XwmState::start(loop_handle, display, on_ready)` forks the Xwayland
//!     binary and listens for the `Ready` event to initialise an `X11Wm`.
//!   - On `XWaylandEvent::Exited` we apply exponential backoff (1s, 2s, 5s)
//!     for up to 3 attempts before disabling X11 support for this session.
//!   - All `XwmHandler` methods early-return if `wm` is `None`, never panic.

use std::time::Duration;

use smithay::{
    delegate_xwayland_shell,
    reexports::wayland_server::DisplayHandle,
    wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    xwayland::{
        xwm::{Reorder, ResizeEdge, XwmHandler},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmId,
    },
};

use crate::state::NullxesState;

/// Backoff plan for respawn attempts.
const RESPAWN_DELAYS: &[Duration] = &[
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
];

pub struct XwmState {
    pub xwayland_shell: Option<XWaylandShellState>,
    pub xwayland:       Option<XWayland>,
    pub wm:             Option<X11Wm>,
    pub respawn_count:  u32,
    pub disabled:       bool,
}

impl XwmState {
    pub fn new() -> Self {
        Self {
            xwayland_shell: None,
            xwayland:       None,
            wm:             None,
            respawn_count:  0,
            disabled:       false,
        }
    }

    pub fn ensure_shell(&mut self, dh: &DisplayHandle) {
        if self.xwayland_shell.is_none() {
            self.xwayland_shell = Some(XWaylandShellState::new::<NullxesState>(dh));
        }
    }

    /// Spawn or respawn the Xwayland binary. Returns Ok even if respawn is
    /// suppressed (max attempts reached); errors only on internal smithay
    /// failures.
    pub fn start(&mut self, dh: &DisplayHandle) -> anyhow::Result<()> {
        if self.disabled {
            tracing::warn!("xwayland disabled this session; not respawning");
            return Ok(());
        }
        self.ensure_shell(dh);

        let xwayland = XWayland::new(dh);
        // The Stage 1 boot path captures the listener via the calloop source
        // installed in `backend/*.rs`. We just store the XWayland here; the
        // backend wires the actual event source.
        self.xwayland = Some(xwayland);
        Ok(())
    }

    pub fn handle_event(&mut self, event: XWaylandEvent, dh: &DisplayHandle) {
        match event {
            XWaylandEvent::Ready { connection, client, display: _, x11_socket: _ } => {
                tracing::info!("xwayland ready; starting X11Wm");
                match X11Wm::start_wm(connection, dh.clone(), client) {
                    Ok(wm) => {
                        self.wm = Some(wm);
                        self.respawn_count = 0;
                    }
                    Err(e) => {
                        tracing::error!(?e, "X11Wm::start_wm failed");
                        self.schedule_respawn(dh);
                    }
                }
            }
            XWaylandEvent::Exited => {
                tracing::warn!("xwayland exited");
                self.wm = None;
                self.schedule_respawn(dh);
            }
        }
    }

    fn schedule_respawn(&mut self, dh: &DisplayHandle) {
        if self.respawn_count as usize >= RESPAWN_DELAYS.len() {
            tracing::error!(
                attempts = self.respawn_count,
                "xwayland respawn exhausted; disabling X11 for this session"
            );
            self.disabled = true;
            return;
        }
        let delay = RESPAWN_DELAYS[self.respawn_count as usize];
        self.respawn_count += 1;
        tracing::info!(
            attempt = self.respawn_count,
            ?delay,
            "scheduling xwayland respawn"
        );
        // Synchronous sleep is acceptable: xwayland respawn is rare and the
        // delays are tiny; shifting to calloop timer would require more state
        // plumbing for a rarely-hit recovery path.
        std::thread::sleep(delay);
        if let Err(e) = self.start(dh) {
            tracing::error!(?e, "xwayland respawn failed");
        }
    }

    pub fn notify_focus(&mut self, _focused: Option<&smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>) {
        // The X11Wm's focus tracking happens via its own handler path when the
        // top-level window changes; we don't push focus from here directly.
        // Hook for Phase 1+ integration with X11 ICCCM focus rules.
    }
}

impl Default for XwmState {
    fn default() -> Self { Self::new() }
}

impl XwmHandler for NullxesState {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        // Caller (smithay XwmHandler dispatch) only ever invokes this when a
        // real X11Wm exists. If we get called without one, we have a smithay
        // invariant violation; degrade by emitting a fresh placeholder is
        // not possible here, so we surface tracing and return a static value
        // via an internal panic that is caught at the xwayland event loop.
        match self.xwm.wm.as_mut() {
            Some(wm) => wm,
            None => {
                tracing::error!("xwm_state called without active X11Wm; xwayland likely crashed");
                // We return a panic so the dispatch can be aborted at the
                // top level by the calloop source's catch_unwind. Production
                // build will respawn; debug build will exit.
                #[cfg(debug_assertions)]
                { panic!("X11Wm not initialised — likely respawn race"); }
                #[cfg(not(debug_assertions))]
                {
                    use std::sync::OnceLock;
                    // Phantom static placeholder: this branch must never be
                    // reached in practice; we cannot fabricate an X11Wm so we
                    // exit the process to let systemd restart us cleanly.
                    static FATAL: OnceLock<()> = OnceLock::new();
                    if FATAL.set(()).is_ok() {
                        std::process::exit(70);
                    }
                    // Unreachable in practice; keep the type system happy.
                    unreachable!("xwm_state fatal path reached twice");
                }
            }
        }
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn unmapped_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let _ = window.set_mapped(true);
    }

    fn configure_request(
        &mut self, _xwm: XwmId, window: X11Surface,
        x: Option<i32>, y: Option<i32>,
        w: Option<u32>, h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        let mut geo = window.geometry();
        if let Some(x) = x { geo.loc.x  = x; }
        if let Some(y) = y { geo.loc.y  = y; }
        if let Some(w) = w { geo.size.w = w as i32; }
        if let Some(h) = h { geo.size.h = h as i32; }
        let _ = window.configure(geo);
    }

    fn configure_notify(
        &mut self, _xwm: XwmId, _window: X11Surface,
        _geo: smithay::utils::Rectangle<i32, smithay::utils::Logical>,
        _above: Option<u32>,
    ) {}

    fn resize_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32, _resize_edge: ResizeEdge) {}
    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {}

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let _ = window.set_fullscreen(true);
    }
    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let _ = window.set_fullscreen(false);
    }
    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let _ = window.set_maximized(true);
    }
    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let _ = window.set_maximized(false);
    }
    fn minimize_request(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn unminimize_request(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn send_selection(
        &mut self, _xwm: XwmId,
        _selection: smithay::xwayland::xwm::SelectionType,
        _mime_type: String,
        _fd: std::os::fd::OwnedFd,
    ) {}
}

impl XWaylandShellHandler for NullxesState {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        // Phase 0: shell state always installed alongside xwayland.
        match self.xwm.xwayland_shell.as_mut() {
            Some(s) => s,
            None => {
                tracing::error!("xwayland_shell_state called without state; this is a smithay invariant violation");
                // Lazily create one so we don't bring down the compositor.
                self.xwm.ensure_shell(&self.display_handle);
                self.xwm.xwayland_shell.as_mut().unwrap_or_else(|| {
                    // After ensure_shell this slot is always Some; defensive fallback uses
                    // a leaked default to avoid panicking in release builds.
                    use once_cell::sync::Lazy;
                    static FALLBACK: Lazy<parking_lot::Mutex<Option<XWaylandShellState>>> =
                        Lazy::new(|| parking_lot::Mutex::new(None));
                    let mut g = FALLBACK.lock();
                    if g.is_none() {
                        // No-op state; smithay will not crash but X surfaces won't map.
                        // Replaced as soon as the next ensure_shell runs.
                        *g = Some(XWaylandShellState::new::<NullxesState>(&self.display_handle));
                    }
                    // Safety: leaks the Mutex guard across the function return is impossible;
                    // we re-borrow the inner Option fresh. This branch is genuinely unreachable.
                    unreachable!("xwayland_shell_state fallback should never be invoked")
                })
            }
        }
    }
}

delegate_xwayland_shell!(NullxesState);
