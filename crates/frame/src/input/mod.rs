//! Input dispatch — keyboard, pointer, touch.
//!
//! Translates `smithay::backend::input::InputEvent` into:
//!   - keyboard input on `seat.get_keyboard()`,
//!   - keybind interception (Super+1..9, Super+Q, Super+L, Super+Enter, etc.),
//!   - pointer input + focus updates,
//!   - cursor clamping to the active output.
//!
//! No `unwrap()` in this file — all seat lookups and surface lookups are
//! `let Some(...) else { return; }` guarded.

pub mod move_grab;
pub mod keybind;

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend,
        InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
        PointerMotionEvent,
    },
    desktop::WindowSurfaceType,
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle, SERIAL_COUNTER},
};

use crate::state::NullxesState;

impl NullxesState {
    pub fn process_input_event<B: InputBackend>(&mut self, event: InputEvent<B>) {
        // While locked, pointer/keyboard go to lock surface only — drop most events
        // here; smithay routes them via lock surface focus already, but we also
        // suppress keybinds.
        let locked = self.lock.is_some();

        match event {
            InputEvent::Keyboard { event } => self.handle_keyboard(event, locked),
            InputEvent::PointerMotion { event } if !locked => self.handle_pointer_motion(event),
            InputEvent::PointerMotionAbsolute { event } if !locked => self.handle_pointer_motion_absolute(event),
            InputEvent::PointerButton { event } if !locked => self.handle_pointer_button(event),
            InputEvent::PointerAxis { event } if !locked => self.handle_pointer_axis(event),
            _ => {}
        }
    }

    fn handle_keyboard<B: InputBackend>(&mut self, event: B::KeyboardKeyEvent, locked: bool) {
        let serial = SERIAL_COUNTER.next_serial();
        let time   = event.time_msec();
        let key    = event.key_code();
        let state  = event.state();

        let Some(kbd) = self.seat.get_keyboard() else { return; };
        let _ = kbd.input::<(), _>(self, key, state, serial, time, |state, mods, keysym| {
            if locked {
                // Forward all keys to the locked client; never intercept.
                return FilterResult::Forward;
            }
            if state.shutdown_reason.is_some() {
                return FilterResult::Forward;
            }
            if matches!(state.lock, Some(_)) {
                return FilterResult::Forward;
            }
            if !matches!(event_state(keysym), ()) {
                // placeholder — keep type happy
            }
            // Delegate to the keybind module for compositor shortcuts.
            keybind::handle_keybind(state, mods, keysym)
        });
    }

    fn handle_pointer_motion<B: InputBackend>(&mut self, event: B::PointerMotionEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        let delta  = event.delta();
        let Some(ptr) = self.seat.get_pointer() else { return; };
        let mut loc = ptr.current_location();
        loc += delta;
        loc = self.clamp_pointer(loc);

        let under = self.surface_under(loc);
        ptr.motion(self, under, &MotionEvent { location: loc, serial, time: event.time_msec() });
        ptr.frame(self);
    }

    fn handle_pointer_motion_absolute<B: InputBackend>(&mut self, event: B::PointerMotionAbsoluteEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        let geo = self
            .primary_output
            .as_ref()
            .and_then(|p| self.active_space().output_geometry(&p.output))
            .unwrap_or_else(|| Rectangle::from_loc_and_size((0, 0), (1920, 1080)));
        let loc = Point::<f64, Logical>::from((
            event.x_transformed(geo.size.w) + geo.loc.x as f64,
            event.y_transformed(geo.size.h) + geo.loc.y as f64,
        ));
        let under = self.surface_under(loc);
        let Some(ptr) = self.seat.get_pointer() else { return; };
        ptr.motion(self, under, &MotionEvent { location: loc, serial, time: event.time_msec() });
        ptr.frame(self);
    }

    fn handle_pointer_button<B: InputBackend>(&mut self, event: B::PointerButtonEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        let button = event.button_code();
        let state  = event.state();
        let Some(ptr) = self.seat.get_pointer() else { return; };
        let loc = ptr.current_location();

        if state == ButtonState::Pressed {
            let active_idx = self.workspace_mgr.active_index();
            let space = self.workspaces.active_mut(active_idx);
            if let Some((window, _pt)) = space.element_under(loc).map(|(w, p)| (w.clone(), p)) {
                let wl = window.wl_surface().map(|s| s.into_owned());
                space.raise_element(&window, true);
                if let Some(kbd) = self.seat.get_keyboard() {
                    kbd.set_focus(self, wl.as_ref(), serial);
                }
            } else {
                if let Some(kbd) = self.seat.get_keyboard() {
                    kbd.set_focus(self, None, serial);
                }
            }
        }

        ptr.button(self, &ButtonEvent { button, state, serial, time: event.time_msec() });
        ptr.frame(self);
    }

    fn handle_pointer_axis<B: InputBackend>(&mut self, event: B::PointerAxisEvent) {
        let src = event.source();
        let mut frame = AxisFrame::new(event.time_msec()).source(src);
        if let Some(v) = event.amount(Axis::Vertical) {
            frame = frame.value(Axis::Vertical, v);
        }
        if let Some(d) = event.amount_v120(Axis::Vertical) {
            frame = frame.v120(Axis::Vertical, d as i32);
        }
        if let Some(v) = event.amount(Axis::Horizontal) {
            frame = frame.value(Axis::Horizontal, v);
        }
        if src == AxisSource::Finger {
            if event.amount(Axis::Vertical) == Some(0.0)   { frame = frame.stop(Axis::Vertical); }
            if event.amount(Axis::Horizontal) == Some(0.0) { frame = frame.stop(Axis::Horizontal); }
        }
        let Some(ptr) = self.seat.get_pointer() else { return; };
        ptr.axis(self, frame);
        ptr.frame(self);
    }

    fn surface_under(
        &self,
        loc: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        // Layer surfaces are checked first if they cover this point at the
        // top/overlay layer (active layer surfaces with input focus).
        if let Some(primary) = self.primary_output.as_ref() {
            let map = smithay::desktop::layer_map_for_output(&primary.output);
            if let Some(layer) = map.layer_under(
                smithay::wayland::shell::wlr_layer::Layer::Overlay,
                loc,
            ).or_else(|| map.layer_under(
                smithay::wayland::shell::wlr_layer::Layer::Top,
                loc,
            )) {
                let layer_loc = map.layer_geometry(layer).map(|g| g.loc).unwrap_or_default();
                if let Some((s, sp)) = layer.surface_under(loc - layer_loc.to_f64(), WindowSurfaceType::ALL) {
                    return Some((s, sp + layer_loc));
                }
            }
        }
        let space = self.active_space();
        space.element_under(loc).and_then(|(window, point)| {
            window
                .surface_under(point.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, sp)| (s, sp))
        })
    }

    fn clamp_pointer(&self, mut loc: Point<f64, Logical>) -> Point<f64, Logical> {
        let space = self.active_space();
        if let Some(geo) = self
            .primary_output
            .as_ref()
            .and_then(|p| space.output_geometry(&p.output))
        {
            loc.x = loc.x.clamp(geo.loc.x as f64, (geo.loc.x + geo.size.w) as f64);
            loc.y = loc.y.clamp(geo.loc.y as f64, (geo.loc.y + geo.size.h) as f64);
        }
        loc
    }
}

fn event_state(_k: smithay::input::keyboard::Keysym) {}
