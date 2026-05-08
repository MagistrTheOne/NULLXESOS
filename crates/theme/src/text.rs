//! CPU-side text renderer — fontdue rasterization into ARGB8888 shm buffers.
//!
//! Design rationale for Stage 1:
//!   fontdue rasterizes directly on the CPU into a Vec<u8> coverage bitmap.
//!   We alpha-blend that bitmap into the pixel buffer in-place. No GPU, no atlas.
//!   This is fast enough for panel + launcher (< 200 unique glyphs per frame,
//!   ~microseconds per glyph on a modern CPU).
//!
//! Stage 2 migration path:
//!   Replace the per-frame CPU blend with a GPU texture atlas. The GlyphCache
//!   stays; the draw_text function changes to emit quads instead of blitting
//!   pixels. The API surface (draw_text signature) is stable across the change.

use std::collections::HashMap;
use fontdue::{Font, FontSettings, Metrics};

// ── Font loading ─────────────────────────────────────────────────────────────

/// Loaded font collection.  Pass font bytes from the binary crate via
/// `include_bytes!()` or runtime file loading.
pub struct Fonts {
    pub regular: Font,
    pub medium:  Font,
    pub mono:    Font,
}

impl Fonts {
    pub fn from_bytes(
        regular_bytes: &[u8],
        medium_bytes:  &[u8],
        mono_bytes:    &[u8],
    ) -> Result<Self, String> {
        Ok(Self {
            regular: Font::from_bytes(regular_bytes, FontSettings::default())?,
            medium:  Font::from_bytes(medium_bytes,  FontSettings::default())?,
            mono:    Font::from_bytes(mono_bytes,     FontSettings::default())?,
        })
    }

    /// Try to load fonts from a prioritised list of candidate directories.
    /// Returns `None` if nothing is found.
    pub fn load_from_candidates(base_dir: &std::path::Path) -> Option<Self> {
        let regular = load_candidate(base_dir, &[
            "Inter-Regular.ttf",
            "inter/Inter-Regular.ttf",
            "Inter/Inter-Regular.ttf",
        ])?;
        let medium = load_candidate(base_dir, &[
            "Inter-Medium.ttf",
            "inter/Inter-Medium.ttf",
            "Inter/Inter-Medium.ttf",
        ]).unwrap_or_else(|| regular.clone()); // fall back to regular
        let mono = load_candidate(base_dir, &[
            "JetBrainsMono-Regular.ttf",
            "jetbrains-mono/JetBrainsMono-Regular.ttf",
            "JetBrainsMono/JetBrainsMono-Regular.ttf",
        ])?;

        Self::from_bytes(&regular, &medium, &mono).ok()
    }
}

fn load_candidate(base: &std::path::Path, names: &[&str]) -> Option<Vec<u8>> {
    for name in names {
        let p = base.join(name);
        if let Ok(bytes) = std::fs::read(&p) {
            return Some(bytes);
        }
    }
    // Also search common system font dirs.
    let system_dirs = [
        "/usr/share/fonts/truetype",
        "/usr/share/fonts/opentype",
        "/usr/share/fonts",
        "/usr/local/share/fonts",
    ];
    for dir in &system_dirs {
        for name in names {
            let p = std::path::Path::new(dir).join(name);
            if let Ok(bytes) = std::fs::read(&p) {
                return Some(bytes);
            }
        }
    }
    None
}

// ── Glyph cache ───────────────────────────────────────────────────────────────

/// Cached rasterization of a single glyph at one size.
struct CachedGlyph {
    metrics: Metrics,
    bitmap:  Vec<u8>,   // 1 byte per pixel, linear alpha coverage 0..=255
}

/// Key: (char, font variant index 0/1/2, size in half-pixels to avoid float)
type GlyphKey = (char, u8, u32);

pub struct GlyphCache {
    glyphs: HashMap<GlyphKey, CachedGlyph>,
}

impl GlyphCache {
    pub fn new() -> Self {
        Self { glyphs: HashMap::with_capacity(256) }
    }

    fn get_or_rasterize<'a>(
        &'a mut self,
        font:     &Font,
        font_idx: u8,
        ch:       char,
        px:       f32,
    ) -> &'a CachedGlyph {
        // Key uses px * 2 as u32 to store half-pixel precision without float hash.
        let key = (ch, font_idx, (px * 2.0).round() as u32);
        self.glyphs.entry(key).or_insert_with(|| {
            let (metrics, bitmap) = font.rasterize(ch, px);
            CachedGlyph { metrics, bitmap }
        })
    }
}

impl Default for GlyphCache {
    fn default() -> Self { Self::new() }
}

// ── Text renderer ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub enum FontVariant {
    Regular = 0,
    Medium  = 1,
    Mono    = 2,
}

pub struct TextRenderer {
    fonts: Fonts,
    cache: GlyphCache,
}

impl TextRenderer {
    pub fn new(fonts: Fonts) -> Self {
        Self { fonts, cache: GlyphCache::new() }
    }

    /// Draw `text` into a raw ARGB8888 pixel buffer.
    ///
    /// Parameters:
    ///   pixels   — mutable byte slice: BGRA layout (wl_shm ARGB8888 little-endian)
    ///   stride   — bytes per row (= width * 4)
    ///   buf_w/h  — buffer dimensions in pixels
    ///   x, y     — text origin: x is left edge, y is the **baseline**
    ///   px       — font size in pixels
    ///   color    — text colour (theme::Color)
    ///   variant  — Regular / Medium / Mono
    ///
    /// Returns the pixel width of the rendered string.
    pub fn draw_text(
        &mut self,
        pixels:  &mut [u8],
        stride:  usize,
        buf_w:   u32,
        buf_h:   u32,
        x:       i32,
        y:       i32,
        text:    &str,
        px:      f32,
        color:   crate::color::Color,
        variant: FontVariant,
    ) -> i32 {
        let font = match variant {
            FontVariant::Regular => &self.fonts.regular,
            FontVariant::Medium  => &self.fonts.medium,
            FontVariant::Mono    => &self.fonts.mono,
        };
        let fidx = variant as u8;

        let [fr, fg, fb, _] = color.to_u8();
        let mut cursor_x = x;

        for ch in text.chars() {
            let glyph = self.cache.get_or_rasterize(font, fidx, ch, px);
            let m = &glyph.metrics;

            if m.width == 0 {
                cursor_x += m.advance_width as i32;
                continue;
            }

            // Top-left of glyph bitmap in screen coords.
            // baseline y → screen_y such that:
            //   glyph spans [y - ymin - height, y - ymin) vertically
            let glyph_top = y - m.ymin - m.height as i32;
            let glyph_left = cursor_x + m.xmin;

            for row in 0..m.height {
                let py = glyph_top + row as i32;
                if py < 0 || py >= buf_h as i32 { continue; }
                let py = py as usize;

                for col in 0..m.width {
                    let px_x = glyph_left + col as i32;
                    if px_x < 0 || px_x >= buf_w as i32 { continue; }
                    let px_x = px_x as usize;

                    let coverage = glyph.bitmap[row * m.width + col];
                    if coverage == 0 { continue; }

                    let idx = py * stride + px_x * 4;
                    // Porter-Duff source-over in pre-multiplied-alpha form.
                    // For opaque destination (a=255 always in wl_shm):
                    let src_a = coverage as u32;
                    let dst_a = 255 - src_a;
                    pixels[idx]     = ((pixels[idx]     as u32 * dst_a + fb as u32 * src_a) / 255) as u8;
                    pixels[idx + 1] = ((pixels[idx + 1] as u32 * dst_a + fg as u32 * src_a) / 255) as u8;
                    pixels[idx + 2] = ((pixels[idx + 2] as u32 * dst_a + fr as u32 * src_a) / 255) as u8;
                    pixels[idx + 3] = 255;
                }
            }

            cursor_x += m.advance_width as i32;
        }

        cursor_x - x
    }

    /// Measure text width without rendering. Uses the cache.
    pub fn measure_text(&mut self, text: &str, px: f32, variant: FontVariant) -> i32 {
        let font = match variant {
            FontVariant::Regular => &self.fonts.regular,
            FontVariant::Medium  => &self.fonts.medium,
            FontVariant::Mono    => &self.fonts.mono,
        };
        let fidx = variant as u8;
        let mut w = 0i32;
        for ch in text.chars() {
            let g = self.cache.get_or_rasterize(font, fidx, ch, px);
            w += g.metrics.advance_width as i32;
        }
        w
    }
}
