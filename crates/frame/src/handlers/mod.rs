//! smithay protocol handler implementations.
//!
//! One module per protocol. Each is a thin façade that forwards into the
//! appropriate state object on `NullxesState` and applies our own desktop
//! semantics (which workspace the new toplevel lands in, where layer surfaces
//! anchor, etc.).

pub mod compositor;
pub mod data_device;
pub mod layer_shell;
pub mod output;
pub mod seat;
pub mod session_lock;
pub mod shm;
pub mod xdg_shell;
