//! Backend selection.
//!
//! - `winit`: development backend running inside an existing X11 / Wayland session.
//! - `drm`:   production backend on bare metal via libseat + udev + libinput.

#[cfg(feature = "winit")]
pub mod winit;

#[cfg(feature = "drm")]
pub mod drm;
