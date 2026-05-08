//! Server-side `wlr-layer-shell` — required for LAUNCHER, NX-NOTIF, and any
//! third-party panel/dock client.
//!
//! On `new_layer_surface`, we anchor it to the primary output and add it to
//! that output's `LayerMap`. The render path consults the LayerMap on each
//! frame so layer surfaces composite at the right z-order (background → bottom →
//! windows → top → overlay).

use smithay::{
    delegate_layer_shell,
    desktop::{layer_map_for_output, LayerSurface},
    output::Output,
    reexports::wayland_server::protocol::wl_output::WlOutput,
    wayland::shell::wlr_layer::{
        LayerSurface as WlrLayerSurface, WlrLayerShellHandler, WlrLayerShellState,
    },
};

use crate::state::NullxesState;

impl WlrLayerShellHandler for NullxesState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface:   WlrLayerSurface,
        wl_output: Option<WlOutput>,
        _layer:    smithay::wayland::shell::wlr_layer::Layer,
        namespace: String,
    ) {
        // Pick output: explicit request first, else primary.
        let output: Option<Output> = match wl_output {
            Some(wl) => Output::from_resource(&wl),
            None => self.primary_output.as_ref().map(|p| p.output.clone()),
        };

        let Some(output) = output else {
            tracing::warn!(%namespace, "layer surface arrived before any output ready; closing");
            surface.send_close();
            return;
        };

        let layer_surface = LayerSurface::new(surface, namespace.clone());
        let mut map = layer_map_for_output(&output);
        if let Err(e) = map.map_layer(&layer_surface) {
            tracing::error!(?e, %namespace, "layer_map.map_layer failed");
            return;
        }
        tracing::info!(%namespace, output = %output.name(), "layer surface mapped");
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        if let Some(primary) = self.primary_output.as_ref() {
            let mut map = layer_map_for_output(&primary.output);
            let layer = map
                .layers()
                .find(|l| l.layer_surface() == &surface)
                .cloned();
            if let Some(layer) = layer {
                map.unmap_layer(&layer);
            }
        }
    }
}

delegate_layer_shell!(NullxesState);
