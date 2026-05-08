//! NULLXES type system — Inter for UI, JetBrains Mono for code.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFamily {
    Inter,
    JetBrainsMono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Regular = 400,
    Medium  = 500,
    SemiBold= 600,
    Bold    = 700,
}

/// A fully-resolved text style.
#[derive(Debug, Clone, Copy)]
pub struct TextStyle {
    pub family:      FontFamily,
    pub weight:      FontWeight,
    pub size_px:     f32,
    pub line_height: f32,  // px
    pub tracking:    f32,  // extra letter-spacing in px (usually 0)
}

impl TextStyle {
    pub const fn new(size_px: f32, line_height: f32, weight: FontWeight) -> Self {
        Self {
            family: FontFamily::Inter,
            weight,
            size_px,
            line_height,
            tracking: 0.0,
        }
    }
}

// ── Type scale ────────────────────────────────────────────────────────────────
pub const CAPTION:      TextStyle = TextStyle::new(11.0, 14.0, FontWeight::Regular);
pub const BODY_SMALL:   TextStyle = TextStyle::new(13.0, 18.0, FontWeight::Regular);
pub const BODY:         TextStyle = TextStyle::new(14.0, 20.0, FontWeight::Regular);
pub const BODY_LARGE:   TextStyle = TextStyle::new(15.0, 22.0, FontWeight::Regular);

pub const LABEL: TextStyle = TextStyle {
    family:      FontFamily::Inter,
    weight:      FontWeight::Medium,
    size_px:     12.0,
    line_height: 16.0,
    tracking:    0.5,
};

pub const HEADING_4: TextStyle = TextStyle::new(16.0, 22.0, FontWeight::SemiBold);
pub const HEADING_3: TextStyle = TextStyle::new(19.0, 26.0, FontWeight::SemiBold);
pub const HEADING_2: TextStyle = TextStyle::new(24.0, 32.0, FontWeight::Bold);
pub const HEADING_1: TextStyle = TextStyle::new(32.0, 40.0, FontWeight::Bold);

pub const CODE: TextStyle = TextStyle {
    family:      FontFamily::JetBrainsMono,
    weight:      FontWeight::Regular,
    size_px:     13.0,
    line_height: 20.0,
    tracking:    0.0,
};

/// Font file paths relative to the asset directory.
pub mod paths {
    pub const INTER_VARIABLE:        &str = "fonts/Inter-Variable.ttf";
    pub const JETBRAINS_MONO_REGULAR:&str = "fonts/JetBrainsMono-Regular.ttf";
    pub const JETBRAINS_MONO_BOLD:   &str = "fonts/JetBrainsMono-Bold.ttf";
}
