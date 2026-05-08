//! Compositor render loop.
//!
//! Per frame:
//!   1. Determine the active workspace and its `Space<Window>`.
//!   2. Collect render-elements from the active space + the layer map for
//!      the primary output (background → bottom → space → top → overlay).
//!   3. Render the in-process panel bitmap as a `MemoryRenderBufferRenderElement`
//!      anchored to the bottom of the output (after toplevels, before overlay).
//!   4. Run `OutputDamageTracker::render_output` and submit.
//!   5. Send frame callbacks to mapped surfaces.
//!
//! Migration: replacing GlesRenderer with a Vulkan renderer keeps this entire
//! function shape; only `RenderElement` types vary.

use std::cell::RefCell;
use std::sync::Arc;

use parking_lot::Mutex;
use smithay::{
    backend::{
        renderer::{
            damage::OutputDamageTracker,
            element::{
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
                surface::WaylandSurfaceRenderElement,
                AsRenderElements, Kind,
            },
            gles::GlesRenderer,
        },
        winit::WinitGraphicsBackend,
    },
    desktop::{layer_map_for_output, space::SpaceRenderElements},
    output::Output,
    utils::{Physical, Point, Scale, Transform},
};

use crate::state::NullxesState;

const BG: [f32; 4] = [0.0392, 0.0392, 0.0392, 1.0]; // theme::color::BG

thread_local! {
    static DAMAGE_TRACKER: RefCell<Option<OutputDamageTracker>> = RefCell::new(None);
    static PANEL_MEMORY:   RefCell<Option<MemoryRenderBuffer>>  = RefCell::new(None);
    static PANEL_GEN:      RefCell<u64>                          = RefCell::new(0);
}

#[cfg(feature = "winit")]
pub fn render_frame_winit(
    backend: &mut WinitGraphicsBackend<GlesRenderer>,
    state:   &mut NullxesState,
) {
    let Some(primary) = state.primary_output.as_ref() else { return; };
    let output = primary.output.clone();

    if backend.bind().is_err() {
        tracing::warn!("EGL bind failed — skipping frame");
        return;
    }

    // Update panel bitmap if dirty (workspace changed, module update, etc.)
    update_panel_bitmap(state);

    // Collect elements: layer-shell background → bottom → toplevels → top → overlay → panel
    let elements = collect_elements(backend.renderer(), state, &output);

    DAMAGE_TRACKER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(OutputDamageTracker::from_output(&output));
        }
        let Some(tracker) = slot.as_mut() else { return; };
        let age = backend.buffer_age().unwrap_or(0) as usize;
        match tracker.render_output(backend.renderer(), age, &elements, BG) {
            Ok(result) => {
                let damage = result.damage.map(|d| d.as_slice());
                if let Err(e) = backend.submit(damage) {
                    tracing::warn!(?e, "submit failed");
                }
            }
            Err(e) => {
                tracing::error!(?e, "render_output failed; resetting damage tracker");
                *slot = None;
            }
        }
    });

    state.send_frames_after_render();
}

#[cfg(feature = "drm")]
pub fn render_frame_drm(
    renderer: &mut GlesRenderer,
    state:    &mut NullxesState,
    output:   &Output,
) {
    update_panel_bitmap(state);
    let elements = collect_elements(renderer, state, output);

    DAMAGE_TRACKER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(OutputDamageTracker::from_output(output));
        }
        let Some(tracker) = slot.as_mut() else { return; };
        let age = 0usize; // DRM backend tracks buffer age separately
        if let Err(e) = tracker.render_output(renderer, age, &elements, BG) {
            tracing::error!(?e, "drm render_output failed");
            *slot = None;
        }
    });

    state.send_frames_after_render();
}

fn update_panel_bitmap(state: &mut NullxesState) {
    let primary_w = state
        .primary_output
        .as_ref()
        .and_then(|p| p.output.current_mode().map(|m| m.size.w as u32))
        .unwrap_or(1920);
    state.panel.resize(primary_w);

    let occupied = state.workspaces.occupied();
    state.panel.render(&state.workspace_mgr, &occupied);

    PANEL_GEN.with(|g| {
        let cur = *g.borrow();
        if cur != state.panel.bitmap.generation {
            PANEL_MEMORY.with(|cell| {
                let mut slot = cell.borrow_mut();
                let buf = MemoryRenderBuffer::from_memory(
                    &state.panel.bitmap.pixels,
                    smithay::backend::allocator::Fourcc::Argb8888,
                    (state.panel.bitmap.width as i32, state.panel.bitmap.height as i32),
                    1,
                    Transform::Normal,
                    None,
                );
                *slot = Some(buf);
            });
            *g.borrow_mut() = state.panel.bitmap.generation;
        }
    });
}

#[allow(clippy::type_complexity)]
fn collect_elements(
    renderer: &mut GlesRenderer,
    state:    &NullxesState,
    output:   &Output,
) -> Vec<RenderElementVariant> {
    let mut out: Vec<RenderElementVariant> = Vec::new();

    let scale = output
        .current_scale()
        .fractional_scale();
    let scale = Scale::from(scale);

    // Layer surfaces: background + bottom first (rendered below windows).
    let map = layer_map_for_output(output);
    for layer in map.layers_on(smithay::wayland::shell::wlr_layer::Layer::Background) {
        push_layer_elements(&mut out, renderer, layer, scale, &map);
    }
    for layer in map.layers_on(smithay::wayland::shell::wlr_layer::Layer::Bottom) {
        push_layer_elements(&mut out, renderer, layer, scale, &map);
    }

    // Toplevels (active workspace only).
    let space = state.active_space();
    let space_elems: Vec<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>> =
        space.render_elements_for_output(renderer, output, scale);
    for e in space_elems {
        out.push(RenderElementVariant::Space(e));
    }

    // Top + overlay layers (above windows).
    for layer in map.layers_on(smithay::wayland::shell::wlr_layer::Layer::Top) {
        push_layer_elements(&mut out, renderer, layer, scale, &map);
    }
    for layer in map.layers_on(smithay::wayland::shell::wlr_layer::Layer::Overlay) {
        push_layer_elements(&mut out, renderer, layer, scale, &map);
    }

    // In-process panel — anchored bottom-left, full width.
    PANEL_MEMORY.with(|cell| {
        let slot = cell.borrow();
        let Some(buf) = slot.as_ref() else { return; };
        let panel_h = state.panel.bitmap.height as i32;
        let mode = output.current_mode();
        let out_h = mode.map(|m| m.size.h).unwrap_or(1080);
        let location = Point::<i32, Physical>::from((0, out_h - panel_h));
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            location.to_f64(),
            buf,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            out.push(RenderElementVariant::Memory(elem));
        }
    });

    // Lock surfaces — when locked, the lock buffer covers everything else
    // visually (we still composite below for the unlock transition fade).
    if let Some(lock) = state.lock.as_ref() {
        for surf in &lock.surfaces {
            let surface = surf.wl_surface();
            if let Some(elements) = surface.as_ref().and_then(|wl| {
                Some(<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface
                    as smithay::backend::renderer::element::AsRenderElements<GlesRenderer>>
                    ::render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
                        wl, renderer, (0, 0).into(), scale, 1.0,
                    ))
            }) {
                for e in elements {
                    out.push(RenderElementVariant::Surface(e));
                }
            }
        }
    }

    out
}

fn push_layer_elements(
    out:      &mut Vec<RenderElementVariant>,
    renderer: &mut GlesRenderer,
    layer:    &smithay::desktop::LayerSurface,
    scale:    Scale<f64>,
    map:      &smithay::desktop::LayerMap,
) {
    let geo = map.layer_geometry(layer).unwrap_or_default();
    let elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
        layer.render_elements(renderer, geo.loc.to_physical_precise_round(scale), scale, 1.0);
    for e in elements {
        out.push(RenderElementVariant::Surface(e));
    }
}

// Unified render-element wrapper so we can mix layer surfaces, toplevels,
// and the panel memory buffer in one Vec.
smithay::backend::renderer::element::render_elements! {
    pub RenderElementVariant<R> where R: smithay::backend::renderer::ImportAll + smithay::backend::renderer::ImportMem;
    Surface = WaylandSurfaceRenderElement<R>,
    Space   = SpaceRenderElements<R, WaylandSurfaceRenderElement<R>>,
    Memory  = MemoryRenderBufferRenderElement<R>,
}
