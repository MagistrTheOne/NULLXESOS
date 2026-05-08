//! Wayland connection, event loop, and launcher lifecycle.
//!
//! Process lifecycle:
//!   - On any Esc / Enter / `closed` event we set `should_close` and exit
//!     after the next dispatch returns control. We never block on shutdown.

use anyhow::{Context, Result};
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_compositor::WlCompositor,
        wl_keyboard::{self, WlKeyboard},
        wl_seat::{self, WlSeat},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_buffer::WlBuffer,
        wl_surface::WlSurface,
        wl_registry,
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use theme::Theme;
use wlcommon::{
    keymap::{Action, KeySymbol, KeyboardState},
    shm::{BufferUserData, ShmDoubleBuffer},
    SurfaceKind,
};

use crate::{apps, draw, search::Searcher};

pub struct LauncherState {
    pub shm:        WlShm,
    compositor:     WlCompositor,
    layer_shell:    ZwlrLayerShellV1,
    surface:        Option<WlSurface>,
    layer_surface:  Option<ZwlrLayerSurfaceV1>,
    keyboard:       Option<WlKeyboard>,
    keymap:         KeyboardState,
    buffer:         Option<ShmDoubleBuffer<LauncherState>>,
    text:           Option<theme::text::TextRenderer>,
    theme:          Theme,
    query:          String,
    searcher:       Searcher,
    selected:       usize,
    configured:     bool,
    pub should_close: bool,
}

impl SurfaceKind for LauncherState {}

impl LauncherState {
    fn new(
        compositor:  WlCompositor,
        shm:         WlShm,
        layer_shell: ZwlrLayerShellV1,
        theme:       Theme,
    ) -> Self {
        let apps = apps::scan();
        tracing::info!(count = apps.len(), "loaded .desktop entries");
        Self {
            compositor,
            shm,
            layer_shell,
            surface: None,
            layer_surface: None,
            keyboard: None,
            keymap: KeyboardState::new(),
            buffer: None,
            text: wlcommon::load_default_text_renderer(),
            theme,
            query: String::new(),
            searcher: Searcher::new(apps),
            selected: 0,
            configured: false,
            should_close: false,
        }
    }

    fn init_surface(&mut self, qh: &QueueHandle<Self>) {
        let surface = self.compositor.create_surface(qh, ());
        let ls = self.layer_shell.get_layer_surface(
            &surface, None, Layer::Overlay,
            "nullxes-launcher".to_string(), qh, (),
        );
        ls.set_size(draw::W, draw::H);
        ls.set_anchor(Anchor::empty()); // centred on screen
        ls.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        surface.commit();
        self.surface = Some(surface);
        self.layer_surface = Some(ls);
    }

    fn redraw(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured { return; }
        if self.buffer.is_none() {
            match ShmDoubleBuffer::new(self.shm.clone(), qh.clone(), draw::W, draw::H) {
                Ok(b) => self.buffer = Some(b),
                Err(e) => { tracing::error!(?e, "shm alloc failed"); return; }
            }
        }
        let Some(buf_pool) = self.buffer.as_mut() else { return; };

        let results = self.searcher.query(&self.query, 8);
        let theme = &self.theme;
        let text = self.text.as_mut();
        let query = &self.query;
        let selected = self.selected;

        let drawn = match buf_pool.draw(|pixels, stride, w, h| {
            draw::paint(pixels, stride, w, h, theme, text, query, &results, selected);
        }) {
            Ok(d) => d,
            Err(e) => { tracing::warn!(?e, "shm draw failed (busy?)"); return; }
        };

        let Some(surface) = self.surface.as_ref() else { return; };
        drawn.attach_and_commit(surface);
    }

    fn handle_action(&mut self, action: Action, qh: &QueueHandle<Self>) {
        match action {
            Action::Escape => self.should_close = true,
            Action::Enter  => { self.launch_selected(); self.should_close = true; }
            Action::Backspace => {
                self.query.pop();
                self.selected = 0;
                self.redraw(qh);
            }
            Action::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.redraw(qh);
                }
            }
            Action::Down => {
                let max = self.searcher.query(&self.query, 8).len().saturating_sub(1);
                if self.selected < max {
                    self.selected += 1;
                    self.redraw(qh);
                }
            }
            _ => {}
        }
    }

    fn handle_text(&mut self, text: &str, qh: &QueueHandle<Self>) {
        self.query.push_str(text);
        self.selected = 0;
        self.redraw(qh);
    }

    fn launch_selected(&self) {
        let results = self.searcher.query(&self.query, 8);
        let Some(app) = results.get(self.selected) else { return; };
        let argv = app.launch_argv();
        let Some((cmd, args)) = argv.split_first() else { return; };
        tracing::info!(cmd, ?args, name = %app.name, "launching");

        // Detach from the launcher process so it survives our exit.
        use std::process::{Command, Stdio};
        let _ = Command::new(cmd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

// ── Dispatch impls ──────────────────────────────────────────────────────────

impl Dispatch<ZwlrLayerSurfaceV1, ()> for LauncherState {
    fn event(state: &mut Self, ls: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event, _: &(),
        _: &Connection, qh: &QueueHandle<Self>) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                state.configured = true;
                ls.ack_configure(serial);
                state.redraw(qh);
            }
            zwlr_layer_surface_v1::Event::Closed => {
                state.should_close = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<WlSeat, ()> for LauncherState {
    fn event(state: &mut Self, seat: &WlSeat, event: wl_seat::Event,
        _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            use wayland_client::WEnum;
            if let WEnum::Value(caps) = capabilities {
                use wayland_client::protocol::wl_seat::Capability;
                if caps.contains(Capability::Keyboard) && state.keyboard.is_none() {
                    state.keyboard = Some(seat.get_keyboard(qh, ()));
                }
            }
        }
    }
}

impl Dispatch<WlKeyboard, ()> for LauncherState {
    fn event(state: &mut Self, _: &WlKeyboard,
        event: wl_keyboard::Event, _: &(),
        _: &Connection, qh: &QueueHandle<Self>) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                use wayland_client::WEnum;
                let WEnum::Value(format) = format else { return; };
                if format != wl_keyboard::KeymapFormat::XkbV1 {
                    tracing::warn!(?format, "unsupported keymap format");
                    return;
                }
                if let Err(e) = state.keymap.apply_keymap(fd, size) {
                    tracing::error!(?e, "apply keymap failed");
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed, mods_latched, mods_locked, group, ..
            } => {
                state.keymap.update_modifiers(mods_depressed, mods_latched, mods_locked, group);
            }
            wl_keyboard::Event::Key { key, state: key_state, .. } => {
                use wayland_client::WEnum;
                let WEnum::Value(ks) = key_state else { return; };
                if ks != wl_keyboard::KeyState::Pressed { return; }
                let symbol = state.keymap.process_key(key);
                match symbol {
                    KeySymbol::Action(a) => state.handle_action(a, qh),
                    KeySymbol::Text(t)   => state.handle_text(&t, qh),
                    KeySymbol::None      => {}
                }
            }
            _ => {}
        }
    }
}

// Boilerplate registry / global dispatchers.
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for LauncherState {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry,
        _: wl_registry::Event, _: &GlobalListContents,
        _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlCompositor, ()> for LauncherState {
    fn event(_: &mut Self, _: &WlCompositor,
        _: wayland_client::protocol::wl_compositor::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShm, ()> for LauncherState {
    fn event(_: &mut Self, _: &WlShm,
        _: wayland_client::protocol::wl_shm::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwlrLayerShellV1, ()> for LauncherState {
    fn event(_: &mut Self, _: &ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlSurface, ()> for LauncherState {
    fn event(_: &mut Self, _: &WlSurface,
        _: wayland_client::protocol::wl_surface::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShmPool, ()> for LauncherState {
    fn event(_: &mut Self, _: &WlShmPool,
        _: wayland_client::protocol::wl_shm_pool::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlBuffer, BufferUserData> for LauncherState {
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
    let (globals, mut queue) = registry_queue_init::<LauncherState>(&conn)?;
    let qh = queue.handle();

    let compositor:  WlCompositor    = globals.bind(&qh, 4..=6, ())?;
    let shm:         WlShm           = globals.bind(&qh, 1..=1, ())?;
    let layer_shell: ZwlrLayerShellV1 = globals.bind(&qh, 1..=4, ())?;
    let _seat:       WlSeat          = globals.bind(&qh, 5..=8, ())?;

    let mut state = LauncherState::new(compositor, shm, layer_shell, theme);
    state.init_surface(&qh);

    while !state.should_close {
        queue.blocking_dispatch(&mut state)?;
    }
    Ok(())
}
