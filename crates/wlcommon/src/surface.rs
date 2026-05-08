//! Surface kind marker — used as a generic parameter on `ShmDoubleBuffer`
//! so that the `Dispatch<WlBuffer, _>` impl is namespaced per client state.
//!
//! Each consumer crate implements `SurfaceKind` for its own state type to
//! make buffer userdata distinct, then derives `Dispatch<WlBuffer, BufferUserData<S>>`.
//!
//! This trait carries no behaviour — it is a phantom marker used solely
//! for type-level discrimination. There is no runtime cost.

/// Marker trait for client state types that own SHM surfaces.
pub trait SurfaceKind: 'static + Send + Sync {}
