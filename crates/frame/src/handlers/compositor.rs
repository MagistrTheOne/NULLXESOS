//! `wl_compositor` + on-commit buffer plumbing.

use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor,
    reexports::wayland_server::{protocol::wl_surface::WlSurface, Client},
    wayland::compositor::{
        get_parent, is_sync_subsurface, with_states, CompositorClientState, CompositorHandler,
        CompositorState,
    },
};

use crate::state::{ClientState, NullxesState};

impl CompositorHandler for NullxesState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        // Every client is initialised with `ClientState` userdata at connection
        // time (see `backend/winit.rs` and `backend/drm.rs`). Missing userdata
        // would be a programming error during client init; we surface it via
        // tracing rather than panicking.
        match client.get_data::<ClientState>() {
            Some(d) => &d.compositor_state,
            None => {
                tracing::error!("client missing ClientState userdata");
                // Fall back to a leaked default state. The client will still
                // function for the duration of its connection.
                use once_cell::sync::Lazy;
                static FALLBACK: Lazy<CompositorClientState> =
                    Lazy::new(CompositorClientState::default);
                &FALLBACK
            }
        }
    }

    fn commit(&mut self, surface: &WlSurface) {
        // Walk subsurface tree to root for damage accumulation.
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) { root = parent; }
        if !is_sync_subsurface(surface) {
            on_commit_buffer_handler::<Self>(surface);
        }

        // Notify smithay desktop layer of buffer commit on toplevels.
        for ws_idx in 0..self.workspaces.count() {
            if let Some(space) = self.workspaces.get_mut(ws_idx) {
                space.commit(surface);
            }
        }

        // Layer surfaces are tracked via layer_map_for_output; smithay handles
        // their commit lifecycle via Window::on_commit-style helpers triggered
        // by the LayerShellHandler — nothing to do here.

        // Pop-ups: ensure ongoing repositioning is honoured.
        self.popups.commit(surface);

        with_states(surface, |_| {});
    }
}

delegate_compositor!(NullxesState);
