//! Selection / DnD plumbing.

use smithay::{
    delegate_data_device,
    wayland::selection::{
        data_device::{
            ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
        },
        SelectionHandler,
    },
};

use crate::state::NullxesState;

impl SelectionHandler for NullxesState {
    type SelectionUserData = ();
}

impl ClientDndGrabHandler for NullxesState {}
impl ServerDndGrabHandler for NullxesState {}

impl DataDeviceHandler for NullxesState {
    fn data_device_state(&self) -> &DataDeviceState { &self.data_device_state }
}

delegate_data_device!(NullxesState);
