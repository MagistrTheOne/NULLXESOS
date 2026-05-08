//! NULLXES spacing grid — all values are multiples of 4 px.

/// Base grid unit: 4 logical pixels.
pub const BASE: u32 = 4;

pub const SP_1:  u32 = BASE;       // 4  — tight internal padding
pub const SP_2:  u32 = BASE * 2;   // 8  — standard component padding
pub const SP_3:  u32 = BASE * 3;   // 12 — gap between related elements
pub const SP_4:  u32 = BASE * 4;   // 16 — gap between components
pub const SP_6:  u32 = BASE * 6;   // 24 — section gap
pub const SP_8:  u32 = BASE * 8;   // 32 — large section gap
pub const SP_12: u32 = BASE * 12;  // 48 — page-level padding
pub const SP_16: u32 = BASE * 16;  // 64 — major structural gap

/// Border radii.
pub mod radius {
    pub const NONE:   u32 = 0;  // progress bars, window borders
    pub const XS:     u32 = 2;  // input fields, small buttons
    pub const SM:     u32 = 4;  // standard buttons, cards
    pub const MD:     u32 = 6;  // large cards, panels
    pub const LG:     u32 = 8;  // dialogs, overlays, launcher
    pub const XL:     u32 = 12; // notifications
}

/// Component heights.
pub mod height {
    pub const COMPACT:     u32 = 32; // dense / developer mode
    pub const COMFORTABLE: u32 = 36; // default
    pub const LARGE:       u32 = 44; // accessible / touch target
    pub const PANEL:       u32 = 48; // bottom panel height
    pub const TITLE_BAR:   u32 = 32; // window title bar
}

/// Minimum click/touch target.
pub const MIN_TARGET: u32 = 32;
