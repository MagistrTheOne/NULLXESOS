//! User-overridable theme configuration, loaded from ~/.config/nullxes/theme.toml.

use serde::{Deserialize, Serialize};
use crate::color::Color;

/// The only user-configurable colour is the accent.
/// Everything else is derived from the fixed dark palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// RRGGBB hex string, e.g. "C0C0C0"
    pub accent_hex: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent_hex: "C0C0C0".into(), // platinum
        }
    }
}

impl Theme {
    pub fn accent(&self) -> Color {
        let hex = u32::from_str_radix(&self.accent_hex, 16).unwrap_or(0xC0C0C0);
        Color::from_hex(hex)
    }

    pub fn load_or_default(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }
}
