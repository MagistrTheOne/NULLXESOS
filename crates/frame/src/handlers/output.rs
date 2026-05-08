//! `wl_output` / `xdg_output` delegation.

use smithay::delegate_output;

use crate::state::NullxesState;

delegate_output!(NullxesState);
