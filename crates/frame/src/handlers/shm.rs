//! `wl_shm` + buffer release.

use smithay::{
    delegate_shm,
    reexports::wayland_server::protocol::wl_buffer::WlBuffer,
    wayland::{
        buffer::BufferHandler,
        shm::{ShmHandler, ShmState},
    },
};

use crate::state::NullxesState;

impl BufferHandler for NullxesState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for NullxesState {
    fn shm_state(&self) -> &ShmState { &self.shm_state }
}

delegate_shm!(NullxesState);
