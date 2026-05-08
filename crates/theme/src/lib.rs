//! NULLXES OS — Design System Tokens
//! Single source of truth for all visual constants.
//! Every UI crate depends on this; nothing hardcodes colours elsewhere.

pub mod color;
pub mod spacing;
pub mod typography;
pub mod motion;
pub mod config;

#[cfg(feature = "render")]
pub mod text;

#[cfg(feature = "cli")]
pub mod export;

pub use color::Color;
pub use config::Theme;
