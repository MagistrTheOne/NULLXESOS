//! Keyboard state for wayland-client UIs.
//!
//! Wraps `xkbcommon::xkb` so each consumer (`launcher`, `nx-lock`, `nx-greet`,
//! `nx-settings`) gets correct UTF-8 keysyms, modifier tracking, and dead-key
//! composition without re-implementing the protocol on every wl_keyboard event.
//!
//! Lifecycle:
//!   `KeyboardState::new()` is empty until `apply_keymap(fd, size)` is called
//!   in response to `wl_keyboard::Event::Keymap`. After that, `update_modifiers`
//!   and `process_key` produce the right `KeySymbol::Char`/`Action` values.
//!
//! This is *not* an IME. CJK / complex input is delegated to xdg input-method
//! protocol once we wire it (post-1.0).

use std::os::fd::{AsRawFd, OwnedFd};

use xkbcommon::xkb;

use crate::error::{ClientError, Result};

/// Logical key event delivered to the UI layer.
#[derive(Debug, Clone)]
pub enum KeySymbol {
    /// Printable text (already filtered for control chars).
    Text(String),
    /// Named action key — caller decides what to do.
    Action(Action),
    /// Pressed key produced no useful symbol.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Up, Down, Left, Right,
    Home, End,
    PageUp, PageDown,
}

pub struct KeyboardState {
    ctx:     xkb::Context,
    keymap:  Option<xkb::Keymap>,
    state:   Option<xkb::State>,
}

impl KeyboardState {
    pub fn new() -> Self {
        Self {
            ctx:    xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            keymap: None,
            state:  None,
        }
    }

    /// Apply a keymap delivered via `wl_keyboard::Event::Keymap { fd, size }`.
    /// `format` must be `XkbV1` per the wayland spec.
    pub fn apply_keymap(&mut self, fd: OwnedFd, size: u32) -> Result<()> {
        // Map the keymap text from the fd. The compositor is required to send
        // a `format = XkbV1` UTF-8 keymap of exactly `size` bytes (incl. NUL).
        // Safety: fd is a valid OwnedFd we own for the duration of this call;
        // mmap is read-only and bounded by the size the compositor sent.
        let mmap = unsafe {
            memmap2::Mmap::map(fd.as_raw_fd())
                .map_err(|e| ClientError::Xkb(format!("keymap mmap: {e}")))?
        };
        let bytes = &mmap[..size.min(mmap.len() as u32) as usize];
        // Strip trailing NUL if present.
        let text = std::str::from_utf8(strip_trailing_nul(bytes))
            .map_err(|e| ClientError::Xkb(format!("keymap utf8: {e}")))?;

        let keymap = xkb::Keymap::new_from_string(
            &self.ctx,
            text.to_owned(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| ClientError::Xkb("keymap compile failed".into()))?;

        let state = xkb::State::new(&keymap);

        self.keymap = Some(keymap);
        self.state  = Some(state);
        // fd is dropped → kernel releases mmap once OwnedFd is dropped at end of scope
        drop(mmap);
        drop(fd);
        Ok(())
    }

    pub fn update_modifiers(
        &mut self,
        mods_depressed: u32,
        mods_latched:   u32,
        mods_locked:    u32,
        group:          u32,
    ) {
        if let Some(state) = &mut self.state {
            state.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
        }
    }

    /// Convert a wayland keycode (already +8 offset per spec) into a logical symbol.
    pub fn process_key(&mut self, wayland_keycode: u32) -> KeySymbol {
        let Some(state) = self.state.as_mut() else {
            return KeySymbol::None;
        };
        let keycode = xkb::Keycode::new(wayland_keycode + 8);
        let keysym = state.key_get_one_sym(keycode);

        if let Some(action) = action_from_keysym(keysym) {
            return KeySymbol::Action(action);
        }

        let utf8 = state.key_get_utf8(keycode);
        if utf8.is_empty() {
            return KeySymbol::None;
        }
        // Filter pure control bytes (Enter, Esc, Tab already handled above).
        if utf8.chars().all(|c| c.is_control()) {
            return KeySymbol::None;
        }
        KeySymbol::Text(utf8)
    }

    pub fn ready(&self) -> bool {
        self.state.is_some()
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self::new()
    }
}

fn action_from_keysym(sym: xkb::Keysym) -> Option<Action> {
    use xkb::keysyms::*;
    match sym.raw() {
        KEY_Return | KEY_KP_Enter        => Some(Action::Enter),
        KEY_Escape                       => Some(Action::Escape),
        KEY_BackSpace                    => Some(Action::Backspace),
        KEY_Delete                       => Some(Action::Delete),
        KEY_Tab | KEY_ISO_Left_Tab       => Some(Action::Tab),
        KEY_Up    | KEY_KP_Up            => Some(Action::Up),
        KEY_Down  | KEY_KP_Down          => Some(Action::Down),
        KEY_Left  | KEY_KP_Left          => Some(Action::Left),
        KEY_Right | KEY_KP_Right         => Some(Action::Right),
        KEY_Home  | KEY_KP_Home          => Some(Action::Home),
        KEY_End   | KEY_KP_End           => Some(Action::End),
        KEY_Page_Up   | KEY_KP_Page_Up   => Some(Action::PageUp),
        KEY_Page_Down | KEY_KP_Page_Down => Some(Action::PageDown),
        _ => None,
    }
}

fn strip_trailing_nul(b: &[u8]) -> &[u8] {
    if b.last() == Some(&0) { &b[..b.len() - 1] } else { b }
}
