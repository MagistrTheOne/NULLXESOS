//! Compositor state owned by the calloop main loop.
//!
//! This is the single source of truth for everything wayland-related during
//! a session. It carries:
//!   - smithay protocol state objects (one per protocol global),
//!   - per-workspace `Space<Window>` containers,
//!   - the optional XWayland WM,
//!   - lock state (Some(LockState) while a session-lock client holds the screen),
//!   - the input seat,
//!   - tokio handles for D-Bus listeners and the IPC server,
//!   - the loop signal used by `signals.rs` and IPC `Quit` to shut down.

use smithay::{
    desktop::{PopupManager, Space, Window},
    input::{Seat, SeatState},
    output::Output,
    reexports::{
        calloop::{channel::Sender, LoopSignal},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            DisplayHandle,
        },
    },
    utils::{Clock, Monotonic},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        session_lock::SessionLockManagerState,
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::ShmState,
    },
};

use crate::{
    config::FrameConfig,
    panel::PanelOverlay,
    workspace::{WorkspaceManager, Workspaces},
    xwm::XwmState,
};

/// Per-output rendering driver type — selects which render path is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Winit,
    Drm,
}

/// Reason for shutdown — propagated to ipc/journald on exit.
#[derive(Debug, Clone, Copy)]
pub enum ShutdownReason {
    Signal(calloop::signals::Signal),
    IpcQuit,
    BackendClosed,
}

impl From<calloop::signals::Signal> for ShutdownReason {
    fn from(s: calloop::signals::Signal) -> Self { ShutdownReason::Signal(s) }
}

/// Single primary output for Stage 1 (multi-output is post-1.0).
pub struct PrimaryOutput {
    pub output: Output,
}

pub struct NullxesState {
    // ── Protocol state ──────────────────────────────────────────────────────
    pub compositor_state:    CompositorState,
    pub xdg_shell_state:     XdgShellState,
    pub layer_shell_state:   WlrLayerShellState,
    pub session_lock_state:  SessionLockManagerState,
    pub shm_state:           ShmState,
    pub output_manager:      OutputManagerState,
    pub seat_state:          SeatState<Self>,
    pub data_device_state:   DataDeviceState,

    // ── Desktop layout ──────────────────────────────────────────────────────
    /// One `Space<Window>` per workspace. Index = workspace id, length = config.workspace.count.
    pub workspaces:          Workspaces,
    pub workspace_mgr:       WorkspaceManager,
    pub popups:              PopupManager,
    pub primary_output:      Option<PrimaryOutput>,

    // ── Lock state ──────────────────────────────────────────────────────────
    pub lock:                Option<LockSession>,

    // ── XWayland ────────────────────────────────────────────────────────────
    pub xwm:                 XwmState,

    // ── In-process panel ────────────────────────────────────────────────────
    pub panel:               PanelOverlay,

    // ── Misc ────────────────────────────────────────────────────────────────
    pub config:              FrameConfig,
    pub clock:               Clock<Monotonic>,
    pub loop_signal:         LoopSignal,
    pub seat:                Seat<Self>,
    pub display_handle:      DisplayHandle,
    pub backend_kind:        BackendKind,
    pub shutdown_reason:     Option<calloop::signals::Signal>,
    pub ipc_request_tx:      Option<Sender<crate::ipc_server::IpcCommand>>,
}

/// Per-output session-lock surface state.
pub struct LockSession {
    pub locker:   smithay::wayland::session_lock::SessionLocker,
    pub surfaces: Vec<smithay::wayland::session_lock::LockSurface>,
}

impl NullxesState {
    pub fn new(
        loop_signal:    LoopSignal,
        display_handle: DisplayHandle,
        config:         FrameConfig,
        backend_kind:   BackendKind,
    ) -> Self {
        let dh = &display_handle;

        let compositor_state    = CompositorState::new::<Self>(dh);
        let xdg_shell_state     = XdgShellState::new::<Self>(dh);
        let layer_shell_state   = WlrLayerShellState::new::<Self>(dh);
        let session_lock_state  = SessionLockManagerState::new::<Self, _>(dh, |_client| true);
        let shm_state           = ShmState::new::<Self>(dh, vec![]);
        let output_manager      = OutputManagerState::new_with_xdg_output::<Self>(dh);
        let mut seat_state      = SeatState::new();
        let data_device_state   = DataDeviceState::new::<Self>(dh);
        let seat                = seat_state.new_wl_seat(dh, "seat0");

        let workspaces = Workspaces::new(config.workspace.count);
        let workspace_mgr = WorkspaceManager::new(config.workspace.count);
        let popups = PopupManager::default();

        Self {
            compositor_state,
            xdg_shell_state,
            layer_shell_state,
            session_lock_state,
            shm_state,
            output_manager,
            seat_state,
            data_device_state,
            workspaces,
            workspace_mgr,
            popups,
            primary_output: None,
            lock: None,
            xwm: XwmState::new(),
            panel: PanelOverlay::new(),
            config,
            clock: Clock::new(),
            loop_signal,
            seat,
            display_handle,
            backend_kind,
            shutdown_reason: None,
            ipc_request_tx: None,
        }
    }

    pub fn active_space(&self) -> &Space<Window> {
        self.workspaces.active(self.workspace_mgr.active_index())
    }

    pub fn active_space_mut(&mut self) -> &mut Space<Window> {
        let idx = self.workspace_mgr.active_index();
        self.workspaces.active_mut(idx)
    }

    /// Send compositor frame callbacks to all visible toplevel + layer surfaces
    /// after a successful render submission. Called from the backend's per-frame
    /// path.
    pub fn send_frames_after_render(&mut self) {
        let now = self.clock.now();
        let active_idx = self.workspace_mgr.active_index();
        let space = self.workspaces.active(active_idx);
        let Some(primary) = self.primary_output.as_ref() else { return; };
        space.elements().for_each(|w| {
            w.send_frame(
                &primary.output,
                now,
                Some(std::time::Duration::from_millis(16)),
                |_, _| Some(primary.output.clone()),
            );
        });
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _id: ClientId) {}
    fn disconnected(&self, _id: ClientId, _reason: DisconnectReason) {}
}
