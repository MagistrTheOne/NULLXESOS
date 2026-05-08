//! NULLXES shared wayland-client primitives.
//!
//! This crate is consumed by every external Wayland client in the system
//! (`launcher`, `slate`, `nx-lock`, `nx-notif`, `nx-settings`, `nx-greet`).
//! It exists so we have one implementation of:
//!
//!  - SHM double-buffered surface allocation with release tracking,
//!  - font candidate discovery,
//!  - keyboard handling via `xkbcommon` (UTF-8 + modifier state),
//!  - common error types.
//!
//! Lifecycle contract per primitive is documented at the top of each module.
//! There is no global state in this crate.

pub mod error;
pub mod fonts;
pub mod keymap;
pub mod shm;
pub mod surface;

pub use error::{ClientError, Result};
pub use fonts::load_default_text_renderer;
pub use keymap::{KeyboardState, KeySymbol};
pub use shm::ShmDoubleBuffer;
pub use surface::SurfaceKind;
