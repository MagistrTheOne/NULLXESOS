//! Wayland state for SLATE.
//!
//! xdg_wm_base + xdg_toplevel client. Keyboard input goes through xkbcommon
//! via `wlcommon::keymap::KeyboardState`; resulting bytes are written to the
//! PTY through `TerminalSession::write`. Resize requests propagate to the
//! PTY winsize via `TerminalSession::resize`.

use std::sync::mpsc;

use anyhow::{Context, Result};
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_keyboard::{self, WlKeyboard},
        wl_seat::{self, WlSeat},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
        wl_registry,
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};

use theme::Theme;
use wlcommon::{
    keymap::{Action, KeySymbol, KeyboardState},
    shm::{BufferUserData, ShmDoubleBuffer},
    SurfaceKind,
};

use crate::{render, term::{snapshot_grid, TerminalSession}};

const DEFAULT_W: u32 = 800;
const DEFAULT_H: u32 = 500;

pub struct SlateState {
    pub shm:        WlShm,
    compositor:     WlCompositor,
    wm_base:        XdgWmBase,
    surface:        Option<WlSurface>,
    xdg_surface:    Option<XdgSurface>,
    toplevel:       Option<XdgToplevel>,
    keyboard:       Option<WlKeyboard>,
    keymap:         KeyboardState,
    buffer:         Option<ShmDoubleBuffer<SlateState>>,
    text:           Option<theme::text::TextRenderer>,
    theme:          Theme,
    width:          u32,
    height:         u32,
    configured:     bool,
    pub should_close: bool,
    term:           Option<TerminalSession>,
    dirty_rx:       Option<mpsc::Receiver<()>>,
    blink_start:    std::time::Instant,
}

impl SurfaceKind for SlateState {}

impl SlateState {
    fn new(compositor: WlCompositor, shm: WlShm, wm_base: XdgWmBase, theme: Theme) -> Self {
        Self {
            compositor, shm, wm_base,
            surface: None,
            xdg_surface: None,
            toplevel: None,
            keyboard: None,
            keymap: KeyboardState::new(),
            buffer: None,
            text: wlcommon::load_default_text_renderer(),
            theme,
            width: DEFAULT_W,
            height: DEFAULT_H,
            configured: false,
            should_close: false,
            term: None,
            dirty_rx: None,
            blink_start: std::time::Instant::now(),
        }
    }

    fn init_surface(&mut self, qh: &QueueHandle<Self>) {
        let surface = self.compositor.create_surface(qh, ());
        let xdg = self.wm_base.get_xdg_surface(&surface, qh, ());
        let toplevel = xdg.get_toplevel(qh, ());
        toplevel.set_title("NULLXES SLATE".into());
        toplevel.set_app_id("os.nullxes.Slate".into());
        surface.commit();
        self.surface = Some(surface);
        self.xdg_surface = Some(xdg);
        self.toplevel = Some(toplevel);
    }

    fn ensure_term(&mut self) {
        if self.term.is_some() { return; }
        let (cols, rows) = render::cell_grid_for_window(self.width, self.height);
        let (tx, rx) = mpsc::channel();
        match TerminalSession::new(cols, rows, render::CELL_W, render::CELL_H, tx) {
            Ok(t)  => { self.term = Some(t); self.dirty_rx = Some(rx); }
            Err(e) => tracing::error!(?e, "failed to start PTY"),
        }
    }

    fn redraw(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured { return; }
        self.ensure_term();
        if self.buffer.is_none() {
            match ShmDoubleBuffer::new(self.shm.clone(), qh.clone(), self.width, self.height) {
                Ok(b) => self.buffer = Some(b),
                Err(e) => { tracing::error!(?e, "shm alloc failed"); return; }
            }
        }
        let Some(pool) = self.buffer.as_mut() else { return; };

        let blink = ((self.blink_start.elapsed().as_millis() % 1000) as f32) / 1000.0;
        let (grid, cursor) = if let Some(t) = self.term.as_ref() {
            let term = t.term.lock();
            let cursor = (term.grid().cursor.point.column.0, term.grid().cursor.point.line.0 as usize);
            (snapshot_grid(&term), cursor)
        } else {
            (Vec::new(), (0, 0))
        };

        let text = self.text.as_mut();
        let drawn = match pool.draw(|pixels, stride, w, h| {
            render::paint(pixels, stride, w, h, text, &grid, cursor, blink);
        }) {
            Ok(d) => d,
            Err(e) => { tracing::warn!(?e, "redraw skipped"); return; }
        };
        let Some(surface) = self.surface.as_ref() else { return; };
        drawn.attach_and_commit(surface);
    }

    fn handle_action(&mut self, action: Action) {
        let Some(term) = self.term.as_ref() else { return; };
        let bytes: &[u8] = match action {
            Action::Enter     => b"\r",
            Action::Backspace => b"\x7f",
            Action::Delete    => b"\x1b[3~",
            Action::Tab       => b"\t",
            Action::Up        => b"\x1b[A",
            Action::Down      => b"\x1b[B",
            Action::Right     => b"\x1b[C",
            Action::Left      => b"\x1b[D",
            Action::Home      => b"\x1b[H",
            Action::End       => b"\x1b[F",
            Action::PageUp    => b"\x1b[5~",
            Action::PageDown  => b"\x1b[6~",
            Action::Escape    => b"\x1b",
        };
        term.write(bytes);
    }

    fn handle_text(&mut self, text: &str) {
        let Some(term) = self.term.as_ref() else { return; };
        term.write(text.as_bytes());
    }
}

// ── Dispatch impls ──────────────────────────────────────────────────────────

impl Dispatch<XdgWmBase, ()> for SlateState {
    fn event(_: &mut Self, base: &XdgWmBase, event: xdg_wm_base::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping { serial } = event { base.pong(serial); }
    }
}

impl Dispatch<XdgSurface, ()> for SlateState {
    fn event(state: &mut Self, surf: &XdgSurface, event: xdg_surface::Event,
        _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let xdg_surface::Event::Configure { serial } = event {
            surf.ack_configure(serial);
            state.configured = true;
            state.redraw(qh);
        }
    }
}

impl Dispatch<XdgToplevel, ()> for SlateState {
    fn event(state: &mut Self, _: &XdgToplevel, event: xdg_toplevel::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                let new_w = if width  > 0 { width  as u32 } else { DEFAULT_W };
                let new_h = if height > 0 { height as u32 } else { DEFAULT_H };
                if new_w != state.width || new_h != state.height {
                    state.width = new_w;
                    state.height = new_h;
                    if let Some(b) = state.buffer.as_mut() {
                        if let Err(e) = b.resize(new_w, new_h) {
                            tracing::error!(?e, "buffer resize failed");
                        }
                    }
                    if let Some(t) = state.term.as_ref() {
                        let (cols, rows) = render::cell_grid_for_window(new_w, new_h);
                        t.resize(cols, rows, render::CELL_W, render::CELL_H);
                    }
                }
            }
            xdg_toplevel::Event::Close => state.should_close = true,
            _ => {}
        }
    }
}

impl Dispatch<WlSeat, ()> for SlateState {
    fn event(state: &mut Self, seat: &WlSeat, event: wl_seat::Event,
        _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(caps) } = event {
            if caps.contains(wayland_client::protocol::wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}

impl Dispatch<WlKeyboard, ()> for SlateState {
    fn event(state: &mut Self, _: &WlKeyboard,
        event: wl_keyboard::Event, _: &(),
        _: &Connection, _: &QueueHandle<Self>) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if let WEnum::Value(format) = format {
                    if format == wl_keyboard::KeymapFormat::XkbV1 {
                        if let Err(e) = state.keymap.apply_keymap(fd, size) {
                            tracing::error!(?e, "apply keymap");
                        }
                    }
                }
            }
            wl_keyboard::Event::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                state.keymap.update_modifiers(mods_depressed, mods_latched, mods_locked, group);
            }
            wl_keyboard::Event::Key { key, state: ks, .. } => {
                if let WEnum::Value(ks) = ks {
                    if ks == wl_keyboard::KeyState::Pressed {
                        match state.keymap.process_key(key) {
                            KeySymbol::Action(a) => state.handle_action(a),
                            KeySymbol::Text(t)   => state.handle_text(&t),
                            KeySymbol::None      => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// Boilerplate.
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for SlateState {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry,
        _: wl_registry::Event, _: &GlobalListContents,
        _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlCompositor, ()> for SlateState {
    fn event(_: &mut Self, _: &WlCompositor,
        _: wayland_client::protocol::wl_compositor::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShm, ()> for SlateState {
    fn event(_: &mut Self, _: &WlShm,
        _: wayland_client::protocol::wl_shm::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlSurface, ()> for SlateState {
    fn event(_: &mut Self, _: &WlSurface,
        _: wayland_client::protocol::wl_surface::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShmPool, ()> for SlateState {
    fn event(_: &mut Self, _: &WlShmPool,
        _: wayland_client::protocol::wl_shm_pool::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlBuffer, BufferUserData> for SlateState {
    fn event(_: &mut Self, _: &WlBuffer,
        event: wayland_client::protocol::wl_buffer::Event,
        data: &BufferUserData, _: &Connection, _: &QueueHandle<Self>) {
        if let wayland_client::protocol::wl_buffer::Event::Release = event {
            data.mark_released();
        }
    }
}

pub fn run(theme: Theme) -> Result<()> {
    let conn = Connection::connect_to_env().context("no Wayland compositor")?;
    let (globals, mut queue) = registry_queue_init::<SlateState>(&conn)?;
    let qh = queue.handle();

    let compositor: WlCompositor = globals.bind(&qh, 4..=6, ())?;
    let shm:        WlShm        = globals.bind(&qh, 1..=1, ())?;
    let wm_base:    XdgWmBase    = globals.bind(&qh, 1..=6, ())?;
    let _seat:      WlSeat       = globals.bind(&qh, 5..=8, ())?;

    let mut state = SlateState::new(compositor, shm, wm_base, theme);
    state.init_surface(&qh);

    while !state.should_close {
        queue.blocking_dispatch(&mut state)?;
        // Drain any dirty notifications and trigger a redraw if needed.
        if let Some(rx) = state.dirty_rx.as_ref() {
            let mut got_any = false;
            while rx.try_recv().is_ok() { got_any = true; }
            if got_any {
                state.redraw(&qh);
            }
        }
    }
    Ok(())
}
