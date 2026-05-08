//! `wl_seat` — keyboard/pointer/touch focus.

use smithay::{
    delegate_seat,
    input::{Seat, SeatHandler, SeatState},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
};

use crate::state::NullxesState;

impl SeatHandler for NullxesState {
    type KeyboardFocus = WlSurface;
    type PointerFocus  = WlSurface;
    type TouchFocus    = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> { &mut self.seat_state }

    fn focus_changed(&mut self, _seat: &Seat<Self>, focused: Option<&WlSurface>) {
        // Update XWayland WM focus mirror so X11 apps see WM_TAKE_FOCUS correctly.
        self.xwm.notify_focus(focused);
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
        // Cursor theming is delivered via theme::cursor in Phase 3; for now
        // we honour client-set images directly through smithay's default path.
    }
}

delegate_seat!(NullxesState);
