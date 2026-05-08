//! NX-SETTINGS Wayland state + Control IPC v1 client.

use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
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

use wlcommon::{
    keymap::{Action, KeySymbol, KeyboardState},
    shm::{BufferUserData, ShmDoubleBuffer},
    SurfaceKind,
};

use crate::{conf, render};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Theme,
    Input,
    Idle,
    About,
}

impl Section {
    pub fn label(&self) -> &'static str {
        match self { Self::Theme => "Theme", Self::Input => "Input", Self::Idle => "Idle", Self::About => "About" }
    }
    pub fn title(&self) -> &'static str {
        match self { Self::Theme => "Theme", Self::Input => "Input", Self::Idle => "Idle & Lock", Self::About => "About" }
    }
}

pub struct UiState {
    pub section: Section,
    pub status:  Option<String>,
}

impl UiState {
    pub fn body_lines(&self) -> Vec<String> {
        let cfg = conf::load_frame();
        match self.section {
            Section::Theme => vec![
                format!("Theme file: {}", conf::theme_path().display()),
                "Edit theme.toml to change accent and surface colours.".into(),
                "After saving, press Enter to push reload to FRAME.".into(),
            ],
            Section::Input => vec![
                format!("xkb_layout      = {:?}", cfg.input.xkb_layout),
                format!("xkb_variant     = {:?}", cfg.input.xkb_variant),
                format!("repeat_delay_ms = {}",   cfg.input.repeat_delay),
                format!("repeat_rate_hz  = {}",   cfg.input.repeat_rate),
                format!("natural_scroll  = {}",   cfg.input.natural_scroll),
                format!("tap_to_click    = {}",   cfg.input.tap_to_click),
            ],
            Section::Idle => vec![
                format!("dim_ms     = {}", cfg.idle.dim_ms),
                format!("lock_ms    = {}", cfg.idle.lock_ms),
                format!("suspend_ms = {}", cfg.idle.suspend_ms),
                "(Edit values in frame.toml then press Enter to reload.)".into(),
            ],
            Section::About => vec![
                format!("NX-SETTINGS {}", env!("CARGO_PKG_VERSION")),
                format!("FRAME socket: {}", ipc::socket_path().display()),
                "Press Enter to ping FRAME for state.".into(),
            ],
        }
    }
}

pub struct SettingsState {
    shm:        WlShm,
    compositor: WlCompositor,
    wm_base:    XdgWmBase,
    surface:    Option<WlSurface>,
    xdg:        Option<XdgSurface>,
    toplevel:   Option<XdgToplevel>,
    keyboard:   Option<WlKeyboard>,
    keymap:     KeyboardState,
    buffer:     Option<ShmDoubleBuffer<SettingsState>>,
    text:       Option<theme::text::TextRenderer>,
    width:      u32,
    height:     u32,
    configured: bool,
    pub should_close: bool,
    pub ui:     Arc<Mutex<UiState>>,
}

impl SurfaceKind for SettingsState {}

impl SettingsState {
    fn new(compositor: WlCompositor, shm: WlShm, wm_base: XdgWmBase) -> Self {
        Self {
            compositor, shm, wm_base,
            surface: None, xdg: None, toplevel: None,
            keyboard: None,
            keymap: KeyboardState::new(),
            buffer: None,
            text: wlcommon::load_default_text_renderer(),
            width: render::W,
            height: render::H,
            configured: false,
            should_close: false,
            ui: Arc::new(Mutex::new(UiState { section: Section::Theme, status: None })),
        }
    }

    fn init_surface(&mut self, qh: &QueueHandle<Self>) {
        let surface = self.compositor.create_surface(qh, ());
        let xdg = self.wm_base.get_xdg_surface(&surface, qh, ());
        let toplevel = xdg.get_toplevel(qh, ());
        toplevel.set_title("NULLXES Settings".into());
        toplevel.set_app_id("os.nullxes.Settings".into());
        surface.commit();
        self.surface = Some(surface);
        self.xdg = Some(xdg);
        self.toplevel = Some(toplevel);
    }

    fn redraw(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured { return; }
        if self.buffer.is_none() {
            match ShmDoubleBuffer::new(self.shm.clone(), qh.clone(), self.width, self.height) {
                Ok(b) => self.buffer = Some(b),
                Err(e) => { tracing::error!(?e, "shm alloc"); return; }
            }
        }
        let Some(pool) = self.buffer.as_mut() else { return; };
        let mut text_taken = self.text.take();
        let ui = self.ui.lock().clone_visible();
        let drawn = match pool.draw(|pixels, stride, w, h| {
            render::paint(pixels, stride, w, h, text_taken.as_mut(), &ui);
        }) {
            Ok(d) => d,
            Err(e) => { tracing::warn!(?e, "redraw"); self.text = text_taken; return; }
        };
        let Some(surface) = self.surface.as_ref() else { self.text = text_taken; return; };
        drawn.attach_and_commit(surface);
        self.text = text_taken;
    }

    fn handle_action(&mut self, action: Action, qh: &QueueHandle<Self>) {
        let mut ui = self.ui.lock();
        match action {
            Action::Up => {
                ui.section = match ui.section {
                    Section::Theme => Section::About,
                    Section::Input => Section::Theme,
                    Section::Idle  => Section::Input,
                    Section::About => Section::Idle,
                };
            }
            Action::Down => {
                ui.section = match ui.section {
                    Section::Theme => Section::Input,
                    Section::Input => Section::Idle,
                    Section::Idle  => Section::About,
                    Section::About => Section::Theme,
                };
            }
            Action::Enter => {
                ui.status = Some("Reloading FRAME…".into());
                let ui_handle = self.ui.clone();
                tokio::spawn(async move {
                    let result = reload_frame().await;
                    let mut g = ui_handle.lock();
                    g.status = Some(match result {
                        Ok(_)  => "FRAME reloaded".into(),
                        Err(e) => format!("FRAME reload failed: {e}"),
                    });
                });
            }
            Action::Escape => self.should_close = true,
            _ => {}
        }
        drop(ui);
        self.redraw(qh);
    }
}

impl UiState {
    fn clone_visible(&self) -> UiState {
        UiState { section: self.section, status: self.status.clone() }
    }
}

async fn reload_frame() -> Result<()> {
    let mut client = ipc::client::Client::connect().await
        .context("connect to FRAME ipc")?;
    let resp = client.request(ipc::request::Request::ReloadConfig).await
        .context("send ReloadConfig")?;
    match resp {
        ipc::response::Response::Ok { .. }  => Ok(()),
        ipc::response::Response::Err { error } => Err(anyhow::anyhow!("{error}")),
    }
}

// ── Dispatch impls ──────────────────────────────────────────────────────────

impl Dispatch<XdgWmBase, ()> for SettingsState {
    fn event(_: &mut Self, base: &XdgWmBase, event: xdg_wm_base::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping { serial } = event { base.pong(serial); }
    }
}
impl Dispatch<XdgSurface, ()> for SettingsState {
    fn event(state: &mut Self, surf: &XdgSurface, event: xdg_surface::Event,
        _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let xdg_surface::Event::Configure { serial } = event {
            surf.ack_configure(serial);
            state.configured = true;
            state.redraw(qh);
        }
    }
}
impl Dispatch<XdgToplevel, ()> for SettingsState {
    fn event(state: &mut Self, _: &XdgToplevel, event: xdg_toplevel::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_toplevel::Event::Close = event { state.should_close = true; }
    }
}
impl Dispatch<WlSeat, ()> for SettingsState {
    fn event(state: &mut Self, seat: &WlSeat, event: wl_seat::Event,
        _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(caps) } = event {
            if caps.contains(wayland_client::protocol::wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}
impl Dispatch<WlKeyboard, ()> for SettingsState {
    fn event(state: &mut Self, _: &WlKeyboard,
        event: wl_keyboard::Event, _: &(),
        _: &Connection, qh: &QueueHandle<Self>) {
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
                            KeySymbol::Action(a) => state.handle_action(a, qh),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// Boilerplate.
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for SettingsState {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents,
        _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlCompositor, ()> for SettingsState { fn event(_: &mut Self, _: &WlCompositor, _: wayland_client::protocol::wl_compositor::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {} }
impl Dispatch<WlShm, ()> for SettingsState { fn event(_: &mut Self, _: &WlShm, _: wayland_client::protocol::wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {} }
impl Dispatch<WlSurface, ()> for SettingsState { fn event(_: &mut Self, _: &WlSurface, _: wayland_client::protocol::wl_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {} }
impl Dispatch<WlShmPool, ()> for SettingsState { fn event(_: &mut Self, _: &WlShmPool, _: wayland_client::protocol::wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {} }
impl Dispatch<WlBuffer, BufferUserData> for SettingsState {
    fn event(_: &mut Self, _: &WlBuffer,
        event: wayland_client::protocol::wl_buffer::Event,
        data: &BufferUserData, _: &Connection, _: &QueueHandle<Self>) {
        if let wayland_client::protocol::wl_buffer::Event::Release = event { data.mark_released(); }
    }
}

pub async fn run() -> Result<()> {
    let conn = Connection::connect_to_env().context("no Wayland compositor")?;
    let (globals, mut queue) = registry_queue_init::<SettingsState>(&conn)?;
    let qh = queue.handle();

    let compositor: WlCompositor = globals.bind(&qh, 4..=6, ())?;
    let shm:        WlShm        = globals.bind(&qh, 1..=1, ())?;
    let wm_base:    XdgWmBase    = globals.bind(&qh, 1..=6, ())?;
    let _seat:      WlSeat       = globals.bind(&qh, 5..=8, ())?;

    let mut state = SettingsState::new(compositor, shm, wm_base);
    state.init_surface(&qh);

    while !state.should_close {
        // Run wayland dispatch on a blocking thread so tokio mainloop keeps
        // serving the IPC client tasks.
        let result = tokio::task::block_in_place(|| queue.blocking_dispatch(&mut state));
        if let Err(e) = result {
            tracing::error!(?e, "wayland dispatch failed");
            break;
        }
    }
    Ok(())
}
