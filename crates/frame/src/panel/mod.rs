//! In-process PANEL.
//!
//! Renders into a CPU pixel buffer (using the existing `theme::text` + the
//! `wlcommon::shm` helpers shape) which the compositor then composites as a
//! `MemoryRenderBufferRenderElement` overlay after the toplevel pass.
//!
//! Why CPU here? The compositor uses `GlesRenderer`, but our text stack is
//! fontdue (CPU). Producing a small CPU buffer once per panel-state-change
//! and uploading it to the GPU is cheaper than rebuilding a glyph atlas
//! on the GPU side every frame. When we replace SHM with Vulkan in 0.2,
//! this module migrates to producing texture-backed elements.

pub mod modules;

use std::sync::Arc;

use parking_lot::Mutex;
use theme::{
    color,
    spacing::height::PANEL,
    text::{FontVariant, TextRenderer},
};

use crate::workspace::WorkspaceManager;

/// Panel pixel buffer, ARGB8888 in-memory (BGRA byte order on little-endian).
pub struct PanelBitmap {
    pub width:  u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub dirty:  bool,
    pub generation: u64,
}

impl PanelBitmap {
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize) * (height as usize) * 4;
        Self {
            width,
            height,
            pixels: vec![0; len],
            dirty: true,
            generation: 0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width == width && self.height == height { return; }
        let len = (width as usize) * (height as usize) * 4;
        self.pixels.clear();
        self.pixels.resize(len, 0);
        self.width = width;
        self.height = height;
        self.dirty = true;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn stride(&self) -> usize { (self.width as usize) * 4 }
}

/// Live panel module state — read by the renderer each frame, written by
/// async tasks (battery, network, volume) on data updates.
#[derive(Default, Clone)]
pub struct ModuleSnapshot {
    pub battery_pct: Option<u8>,
    pub on_ac:       bool,
    pub network:     NetworkSnapshot,
    pub volume_pct:  Option<u8>,
    pub volume_mute: bool,
}

#[derive(Default, Clone)]
pub struct NetworkSnapshot {
    pub kind:      NetworkKind,
    pub connected: bool,
    pub ssid:      Option<String>,
    pub strength:  Option<u8>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum NetworkKind {
    #[default]
    None,
    Wired,
    Wireless,
    Other,
}

pub struct PanelOverlay {
    pub bitmap:     PanelBitmap,
    pub modules:    Arc<Mutex<ModuleSnapshot>>,
    pub text:       Option<TextRenderer>,
}

impl PanelOverlay {
    pub fn new() -> Self {
        Self {
            bitmap:  PanelBitmap::new(1920, PANEL),
            modules: Arc::new(Mutex::new(ModuleSnapshot::default())),
            text:    wlcommon_text_renderer(),
        }
    }

    /// Resize the panel to match output width.
    pub fn resize(&mut self, output_width: u32) {
        self.bitmap.resize(output_width, PANEL);
    }

    /// Render the panel into its bitmap. Cheap: < 1ms for 1920×48 on modern CPUs.
    pub fn render(&mut self, mgr: &WorkspaceManager, occupied: &[usize]) {
        let w = self.bitmap.width;
        let h = self.bitmap.height;
        let stride = self.bitmap.stride();
        let pixels = &mut self.bitmap.pixels[..];

        // Background
        clear(pixels, color::SURFACE_1);
        // Top divider
        wlcommon_hline(pixels, stride, w, h, 0, 0, w as i32, color::DIVIDER);
        // Launcher button area
        fill_rect(pixels, stride, w, h, 0, 0, 48, h, color::SURFACE_2);
        // NX mark
        for (dx, dy) in [(18i32, 16i32), (26, 16), (18, 24), (26, 24)] {
            fill_rect(pixels, stride, w, h, dx, dy, 4, 4, color::ACCENT);
        }

        // Workspace dots
        modules::workspaces::draw(pixels, stride, w, h, mgr.active_index(), occupied);

        // Indicator modules (battery / network / volume / clock)
        if let Some(text) = &mut self.text {
            let snap = self.modules.lock().clone();
            let mut x = w as i32 - 16;
            x = modules::clock::draw(text, pixels, stride, w, h, x, h as i32 / 2 + 5);
            x = modules::battery::draw(text, pixels, stride, w, h, x - 16, h as i32 / 2 + 5, &snap);
            x = modules::network::draw(text, pixels, stride, w, h, x - 16, h as i32 / 2 + 5, &snap);
            let _ = modules::volume::draw(text, pixels, stride, w, h, x - 16, h as i32 / 2 + 5, &snap);
        }

        self.bitmap.dirty = false;
        self.bitmap.generation = self.bitmap.generation.wrapping_add(1);
    }
}

impl Default for PanelOverlay {
    fn default() -> Self { Self::new() }
}

fn wlcommon_text_renderer() -> Option<TextRenderer> {
    // FRAME ships its own font discovery so we don't pull `wlcommon` into the
    // server-side build (which would create a dep cycle). The candidate dirs
    // mirror `wlcommon::fonts::candidate_dirs`.
    let candidates = candidate_dirs();
    for dir in &candidates {
        if let Some(fonts) = theme::text::Fonts::load_from_candidates(dir) {
            tracing::info!(path = %dir.display(), "FRAME panel fonts loaded");
            return Some(TextRenderer::new(fonts));
        }
    }
    tracing::warn!("FRAME panel: fonts not found — clock/indicators will be blank");
    None
}

fn candidate_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(env) = std::env::var("NULLXES_FONT_DIR") {
        dirs.push(std::path::PathBuf::from(env));
    }
    dirs.push(std::path::PathBuf::from("/usr/share/fonts/nullxes"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("assets/fonts"));
        }
    }
    dirs.push(std::path::PathBuf::from("assets/fonts"));
    for d in &[
        "/usr/share/fonts/inter",
        "/usr/share/fonts/Inter",
        "/usr/share/fonts/truetype/inter",
        "/usr/share/fonts/jetbrains-mono",
        "/usr/share/fonts/JetBrainsMono",
        "/usr/share/fonts",
    ] {
        dirs.push(std::path::PathBuf::from(d));
    }
    dirs
}

// ── Local copies of pixel helpers (BGRA in-memory) ─────────────────────────
// We deliberately don't depend on `wlcommon` from FRAME to avoid pulling
// wayland-client into the server build. These mirrors must stay byte-identical
// to `wlcommon::shm::{clear, fill_rect, hline}`.

fn clear(pixels: &mut [u8], c: theme::Color) {
    let [r, g, b, a] = c.to_u8();
    let mut i = 0;
    while i + 3 < pixels.len() {
        pixels[i]     = b;
        pixels[i + 1] = g;
        pixels[i + 2] = r;
        pixels[i + 3] = a;
        i += 4;
    }
}

pub(crate) fn fill_rect(
    pixels: &mut [u8], stride: usize, buf_w: u32, buf_h: u32,
    x: i32, y: i32, w: u32, h: u32,
    color: theme::Color,
) {
    let [r, g, b, a] = color.to_u8();
    let x0 = x.max(0) as usize;
    let y0 = y.max(0) as usize;
    let x1 = (x.saturating_add(w as i32)).min(buf_w as i32).max(0) as usize;
    let y1 = (y.saturating_add(h as i32)).min(buf_h as i32).max(0) as usize;
    if x0 >= x1 || y0 >= y1 { return; }

    if a == 255 {
        for py in y0..y1 {
            let row = py * stride;
            for px in x0..x1 {
                let i = row + px * 4;
                pixels[i]     = b;
                pixels[i + 1] = g;
                pixels[i + 2] = r;
                pixels[i + 3] = 255;
            }
        }
    } else {
        let src_a = a as u32;
        let dst_a = 255 - src_a;
        for py in y0..y1 {
            let row = py * stride;
            for px in x0..x1 {
                let i = row + px * 4;
                pixels[i]     = ((pixels[i]     as u32 * dst_a + b as u32 * src_a) / 255) as u8;
                pixels[i + 1] = ((pixels[i + 1] as u32 * dst_a + g as u32 * src_a) / 255) as u8;
                pixels[i + 2] = ((pixels[i + 2] as u32 * dst_a + r as u32 * src_a) / 255) as u8;
                pixels[i + 3] = 255;
            }
        }
    }
}

pub(crate) fn wlcommon_hline(
    pixels: &mut [u8], stride: usize, buf_w: u32, buf_h: u32,
    y: i32, x0: i32, x1: i32, color: theme::Color,
) {
    if x1 <= x0 { return; }
    fill_rect(pixels, stride, buf_w, buf_h, x0, y, (x1 - x0) as u32, 1, color);
}
