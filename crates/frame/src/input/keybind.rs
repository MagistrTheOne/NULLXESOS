//! Compositor-level keybindings.
//!
//! Bindings (default; user-overridable in 0.2 via `frame.toml`):
//!   - Super (release):       open launcher (idempotent via `launcher.lock`)
//!   - Super + 1..9:          switch to workspace N
//!   - Super + Shift + 1..9:  move focused window to workspace N
//!   - Super + Right / Left:  next / prev workspace
//!   - Super + Q:             close focused window
//!   - Super + L:             lock session (spawns `nullxes-lock`)
//!   - Super + Return:        spawn default terminal (`nullxes-slate`)
//!
//! All shortcuts are intercepted (not forwarded). All other key events are
//! forwarded to the focused client.

use smithay::input::keyboard::{keysyms as KeySyms, FilterResult, Keysym, ModifiersState};

use crate::state::NullxesState;
use crate::workspace::WindowId;

pub fn handle_keybind(
    state: &mut NullxesState,
    mods:  &ModifiersState,
    keysym: Keysym,
) -> FilterResult<()> {
    let super_held = mods.logo;
    let shift_held = mods.shift;

    if !super_held {
        return FilterResult::Forward;
    }

    match keysym.raw() {
        KeySyms::KEY_1..=KeySyms::KEY_9 => {
            let idx = (keysym.raw() - KeySyms::KEY_1) as usize;
            if shift_held {
                if let Some((src, target, id)) = state.workspace_mgr.plan_move_focused(idx) {
                    move_window_between_workspaces(state, src, target, id);
                }
            } else {
                state.workspace_mgr.switch_to(idx);
            }
            return FilterResult::Intercept(());
        }
        KeySyms::KEY_Right => {
            let wrap = state.config.workspace.wrap;
            state.workspace_mgr.switch_next(wrap);
            return FilterResult::Intercept(());
        }
        KeySyms::KEY_Left => {
            let wrap = state.config.workspace.wrap;
            state.workspace_mgr.switch_prev(wrap);
            return FilterResult::Intercept(());
        }
        KeySyms::KEY_q | KeySyms::KEY_Q => {
            close_focused_window(state);
            return FilterResult::Intercept(());
        }
        KeySyms::KEY_l | KeySyms::KEY_L => {
            spawn_detached(&state.config.keybindings.lock_binary);
            return FilterResult::Intercept(());
        }
        KeySyms::KEY_Return | KeySyms::KEY_KP_Enter => {
            spawn_detached("nullxes-slate");
            return FilterResult::Intercept(());
        }
        // Super alone (no modifier and just Super press) → launcher.
        // We trigger on Super_L / Super_R press; the keyboard layer reports
        // these as plain keysyms with logo=true.
        KeySyms::KEY_Super_L | KeySyms::KEY_Super_R => {
            spawn_detached(&state.config.keybindings.launcher_binary);
            return FilterResult::Intercept(());
        }
        _ => {}
    }
    FilterResult::Forward
}

pub fn move_window_between_workspaces(
    state:  &mut NullxesState,
    src:    usize,
    target: usize,
    _id:    WindowId,
) {
    let Some(src_space) = state.workspaces.get(src) else { return; };
    // Find focused-id-matching window in src space.
    let Some(focused_window) = src_space
        .elements()
        .find(|w| {
            w.wl_surface()
                .map(|s| s.id().protocol_id() as u64 == _id.0)
                .unwrap_or(false)
        })
        .cloned()
    else { return; };
    let loc = src_space.element_location(&focused_window).unwrap_or_default();

    if let Some(src_space) = state.workspaces.get_mut(src) {
        src_space.unmap_elem(&focused_window);
    }
    if let Some(dst_space) = state.workspaces.get_mut(target) {
        dst_space.map_element(focused_window, loc, true);
    }
}

fn close_focused_window(state: &mut NullxesState) {
    let Some(focused_id) = state.workspace_mgr.focused() else { return; };
    let active_idx = state.workspace_mgr.active_index();
    let Some(space) = state.workspaces.get(active_idx) else { return; };
    let window = space
        .elements()
        .find(|w| w.wl_surface().map(|s| s.id().protocol_id() as u64).unwrap_or(0) == focused_id.0)
        .cloned();
    if let Some(w) = window {
        if let Some(top) = w.toplevel() {
            top.send_close();
        }
    }
}

/// Spawn a binary detached from FRAME so it survives our shutdown and does
/// not inherit our stdio.
fn spawn_detached(bin: &str) {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(bin);
    cmd.stdin(Stdio::null());
    if !tracing::enabled!(tracing::Level::DEBUG) {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    match cmd.spawn() {
        Ok(child) => tracing::debug!(bin, pid = child.id(), "spawned"),
        Err(e)    => tracing::warn!(bin, ?e, "spawn failed"),
    }
}
