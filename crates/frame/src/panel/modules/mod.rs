//! Panel modules. Each module produces a deterministic, side-effect-free
//! draw call that takes a snapshot of system state and a pixel buffer.

pub mod battery;
pub mod clock;
pub mod network;
pub mod volume;
pub mod workspaces;
