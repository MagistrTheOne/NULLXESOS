//! Pointer grab implementation for interactive window move.

use smithay::{
    desktop::Window,
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
        GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
        GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
        GrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
        RelativeMotionEvent,
    },
    utils::{Logical, Point},
};

use crate::state::NullxesState;

pub struct MoveSurfaceGrab {
    pub start_data:              GrabStartData<NullxesState>,
    pub window:                  Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab<NullxesState> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data:   &mut NullxesState,
        handle: &mut PointerInnerHandle<'_, NullxesState>,
        _focus: Option<(
            <NullxesState as smithay::input::SeatHandler>::PointerFocus,
            Point<i32, Logical>,
        )>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);
        let delta = event.location - self.start_data.location;
        let new_loc = self.initial_window_location + delta.to_i32_round();
        let active_idx = data.workspace_mgr.active_index();
        let space = data.workspaces.active_mut(active_idx);
        space.map_element(self.window.clone(), new_loc, true);
    }

    fn relative_motion(
        &mut self,
        data:   &mut NullxesState,
        handle: &mut PointerInnerHandle<'_, NullxesState>,
        focus:  Option<(
            <NullxesState as smithay::input::SeatHandler>::PointerFocus,
            Point<i32, Logical>,
        )>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data:   &mut NullxesState,
        handle: &mut PointerInnerHandle<'_, NullxesState>,
        event:  &ButtonEvent,
    ) {
        handle.button(data, event);
        if event.state == smithay::backend::input::ButtonState::Released
            && !handle.current_pressed().contains(&event.button)
        {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>, details: AxisFrame) { handle.axis(data, details); }
    fn frame(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>) { handle.frame(data); }
    fn gesture_swipe_begin(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>, event: &GestureSwipeBeginEvent) { handle.gesture_swipe_begin(data, event); }
    fn gesture_swipe_update(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>, event: &GestureSwipeUpdateEvent) { handle.gesture_swipe_update(data, event); }
    fn gesture_swipe_end(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>, event: &GestureSwipeEndEvent) { handle.gesture_swipe_end(data, event); }
    fn gesture_pinch_begin(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>, event: &GesturePinchBeginEvent) { handle.gesture_pinch_begin(data, event); }
    fn gesture_pinch_update(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>, event: &GesturePinchUpdateEvent) { handle.gesture_pinch_update(data, event); }
    fn gesture_pinch_end(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>, event: &GesturePinchEndEvent) { handle.gesture_pinch_end(data, event); }
    fn gesture_hold_begin(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>, event: &GestureHoldBeginEvent) { handle.gesture_hold_begin(data, event); }
    fn gesture_hold_end(&mut self, data: &mut NullxesState, handle: &mut PointerInnerHandle<'_, NullxesState>, event: &GestureHoldEndEvent) { handle.gesture_hold_end(data, event); }

    fn start_data(&self) -> &GrabStartData<NullxesState> { &self.start_data }
    fn unset(&mut self, _data: &mut NullxesState) {}
}
