//! NULLXES colour palette — matte black surfaces, graphite text, single accent.

use serde::{Deserialize, Serialize};

/// RGBA colour, each channel 0.0–1.0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::rgba(r, g, b, 1.0)
    }

    /// Parse from 0xRRGGBB hex literal (fully opaque).
    pub const fn from_hex(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 8)  & 0xFF) as f32 / 255.0;
        let b = ( hex        & 0xFF) as f32 / 255.0;
        Self::rgba(r, g, b, 1.0)
    }

    /// Returns [r, g, b, a] as u8 array.
    pub fn to_u8(self) -> [u8; 4] {
        [
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        ]
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub fn premultiplied(self) -> [u8; 4] {
        [
            (self.r * self.a * 255.0) as u8,
            (self.g * self.a * 255.0) as u8,
            (self.b * self.a * 255.0) as u8,
            (self.a * 255.0) as u8,
        ]
    }
}

// ── Elevation system ─────────────────────────────────────────────────────────
/// The desktop background. Near-black, not pure — retains depth on LCD.
pub const BG:         Color = Color::from_hex(0x0A0A0A);
/// Level 1: PANEL, status bar.
pub const SURFACE_1:  Color = Color::from_hex(0x111111);
/// Level 2: Windows, cards.
pub const SURFACE_2:  Color = Color::from_hex(0x131313);
/// Level 3: Modals, dialogs.
pub const SURFACE_3:  Color = Color::from_hex(0x161616);
/// Level 4: Tooltips, popovers.
pub const SURFACE_4:  Color = Color::from_hex(0x1C1C1C);
/// Level 5: Context menus, dropdowns.
pub const SURFACE_5:  Color = Color::from_hex(0x1F1F1F);

// ── Dividers ─────────────────────────────────────────────────────────────────
pub const DIVIDER:    Color = Color::from_hex(0x252525);

// ── Text ─────────────────────────────────────────────────────────────────────
pub const TEXT_PRIMARY:   Color = Color::from_hex(0xE8E8E8);
pub const TEXT_SECONDARY: Color = Color::from_hex(0x808080);
pub const TEXT_DISABLED:  Color = Color::from_hex(0x404040);

// ── Accent (default: platinum — user-configurable) ───────────────────────────
pub const ACCENT:         Color = Color::from_hex(0xC0C0C0);
pub const ACCENT_STRONG:  Color = Color::from_hex(0xFFFFFF);

// ── Semantic ─────────────────────────────────────────────────────────────────
pub const DESTRUCTIVE:  Color = Color::from_hex(0xC0392B);
pub const WARNING:      Color = Color::from_hex(0xB07D00);
pub const SUCCESS:      Color = Color::from_hex(0x2E7D52);

// ── Transparency helpers ──────────────────────────────────────────────────────
/// Accent at 15% — selection background.
pub const SELECTION_BG: Color = Color::rgba(0.75, 0.75, 0.75, 0.15);
/// Overlay scrim behind modals.
pub const SCRIM:        Color = Color::rgba(0.0, 0.0, 0.0, 0.60);
