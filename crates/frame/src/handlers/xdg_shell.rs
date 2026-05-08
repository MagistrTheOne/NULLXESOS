//! `xdg_shell` — toplevels, popups, move/resize requests.

use smithay::{
    delegate_xdg_shell,
    desktop::{PopupKind, Window},
    input::Seat,
    reexports::wayland_server::{
        protocol::wl_seat::WlSeat,
        Resource,
    },
    utils::{Logical, Point, Rectangle, Serial},
    wayland::shell::xdg::{
        PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    },
};

use crate::input::move_grab::MoveSurfaceGrab;
use crate::state::NullxesState;
use crate::workspace::WindowId;

impl XdgShellHandler for NullxesState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);

        let active_idx = self.workspace_mgr.active_index();
        let space = self.workspaces.active_mut(active_idx);

        // Place the window in the upper-quarter of the primary output. New
        // toplevels with (0,0) reported size will resize on first configure;
        // we simply reserve a sensible position now.
        let geo = self
            .primary_output
            .as_ref()
            .and_then(|p| space.output_geometry(&p.output))
            .unwrap_or_else(|| Rectangle::from_loc_and_size((0, 0), (1920, 1080)));
        let loc = Point::from((
            geo.loc.x + geo.size.w / 4,
            geo.loc.y + geo.size.h / 4,
        ));
        space.map_element(window.clone(), loc, true);

        let id = window_id(&window);
        self.workspace_mgr.set_focus(Some(id));
        tracing::info!(?id, ?loc, "new toplevel mapped");
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        if let Err(e) = self.popups.track_popup(PopupKind::Xdg(surface)) {
            tracing::warn!(?e, "failed to track popup");
        }
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        let Some(seat) = Seat::<NullxesState>::from_resource(&seat) else { return; };
        let wl = surface.wl_surface().clone();
        let Some(ptr) = seat.get_pointer() else { return; };
        if !ptr.has_grab(serial) { return; }
        let Some(start_data) = ptr.grab_start_data() else { return; };

        if start_data
            .focus
            .as_ref()
            .map_or(true, |(f, _)| !f.id().same_client_as(&wl.id()))
        {
            return;
        }

        let active_idx = self.workspace_mgr.active_index();
        let space = self.workspaces.active_mut(active_idx);
        let Some(window) = space
            .elements()
            .find(|w| w.wl_surface().as_deref() == Some(&wl))
            .cloned()
        else { return; };

        let initial_window_location = space.element_location(&window).unwrap_or_default();
        let grab = MoveSurfaceGrab {
            start_data,
            window,
            initial_window_location,
        };
        ptr.set_grab(self, grab, serial, smithay::input::pointer::Focus::Clear);
    }

    fn resize_request(
        &mut self,
        _surface: ToplevelSurface,
        _seat:    WlSeat,
        _serial:  Serial,
        _edges:   smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge,
    ) {
        // Interactive resize ships in 0.2; current toplevels honour configure-driven resize.
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        surface:    PopupSurface,
        positioner: PositionerState,
        token:      u32,
    ) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
        });
        surface.send_repositioned(token);
        let _ = surface.send_configure();
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let wl = surface.wl_surface();

        // The toplevel can live in any workspace; scan all spaces.
        let mut found_id: Option<WindowId> = None;
        for ws_idx in 0..self.workspaces.count() {
            let Some(space) = self.workspaces.get_mut(ws_idx) else { continue; };
            let window = space
                .elements()
                .find(|w| w.wl_surface().as_deref() == Some(wl))
                .cloned();
            if let Some(w) = window {
                let id = window_id(&w);
                space.unmap_elem(&w);
                found_id = Some(id);
                break;
            }
        }

        if let Some(id) = found_id {
            // Clear focused-id slot if it pointed at this window.
            if self.workspace_mgr.focused() == Some(id) {
                self.workspace_mgr.set_focus(None);
            }
            tracing::debug!(?id, "toplevel destroyed");
        }
    }
}

delegate_xdg_shell!(NullxesState);

pub(crate) fn window_id(window: &Window) -> WindowId {
    let raw = window
        .wl_surface()
        .map(|s| s.id().protocol_id() as u64)
        .unwrap_or(0);
    WindowId(raw)
}
