//! NX-GREET wayland state.

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

use wlcommon::{
    keymap::{Action, KeySymbol, KeyboardState},
    shm::{BufferUserData, ShmDoubleBuffer},
    SurfaceKind,
};

use crate::{ipc::{self, FlowOutcome, Greetd}, render::{self, FocusedField}};

pub struct GreetState {
    shm:        WlShm,
    compositor: WlCompositor,
    wm_base:    XdgWmBase,
    surface:    Option<WlSurface>,
    xdg:        Option<XdgSurface>,
    toplevel:   Option<XdgToplevel>,
    keyboard:   Option<WlKeyboard>,
    keymap:     KeyboardState,
    buffer:     Option<ShmDoubleBuffer<GreetState>>,
    text:       Option<theme::text::TextRenderer>,
    width:      u32,
    height:     u32,
    configured: bool,
    pub should_close: bool,

    greetd:     Option<Greetd>,
    username:   String,
    password:   String,
    prompt:     Option<String>,
    error:      Option<String>,
    focus:      FocusedField,
    pending:    bool,
    awaiting_response: bool,
}

impl SurfaceKind for GreetState {}

impl GreetState {
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
            greetd: None,
            username: String::new(),
            password: String::new(),
            prompt: None,
            error: None,
            focus: FocusedField::Username,
            pending: false,
            awaiting_response: false,
        }
    }

    fn init_surface(&mut self, qh: &QueueHandle<Self>) {
        let surface = self.compositor.create_surface(qh, ());
        let xdg = self.wm_base.get_xdg_surface(&surface, qh, ());
        let toplevel = xdg.get_toplevel(qh, ());
        toplevel.set_title("NULLXES Greeter".into());
        toplevel.set_app_id("os.nullxes.Greet".into());
        toplevel.set_fullscreen(None);
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
        let username = self.username.clone();
        let password_len = self.password.len();
        let prompt = self.prompt.clone();
        let error = self.error.clone();
        let focus = self.focus;
        let pending = self.pending;
        let drawn = match pool.draw(|pixels, stride, w, h| {
            render::paint(pixels, stride, w, h, text_taken.as_mut(),
                &username, password_len, prompt.as_deref(), error.as_deref(),
                focus, pending);
        }) {
            Ok(d) => d,
            Err(e) => { tracing::warn!(?e, "redraw"); self.text = text_taken; return; }
        };
        let Some(surface) = self.surface.as_ref() else { self.text = text_taken; return; };
        drawn.attach_and_commit(surface);
        self.text = text_taken;
    }

    fn submit(&mut self, qh: &QueueHandle<Self>) {
        if self.pending { return; }
        if self.username.trim().is_empty() && self.focus == FocusedField::Username {
            self.error = Some("username required".into());
            self.redraw(qh);
            return;
        }
        if self.greetd.is_none() {
            match Greetd::connect() {
                Ok(g) => self.greetd = Some(g),
                Err(e) => {
                    self.error = Some(format!("greetd unreachable: {e}"));
                    self.redraw(qh);
                    return;
                }
            }
        }
        let Some(g) = self.greetd.as_mut() else { return; };
        self.pending = true;

        let resp = if !self.awaiting_response {
            g.create_session(&self.username)
        } else {
            let pw = std::mem::take(&mut self.password);
            g.post_auth_response(if pw.is_empty() { None } else { Some(pw) })
        };

        match resp.map(ipc::classify) {
            Ok(FlowOutcome::Prompt(text, _ty)) => {
                self.prompt = Some(text);
                self.awaiting_response = true;
                self.focus = FocusedField::Password;
                self.error = None;
                self.pending = false;
            }
            Ok(FlowOutcome::Success) => {
                // Tell greetd to start the session; greetd will exec us out.
                if let Some(g) = self.greetd.as_mut() {
                    let _ = g.start_session();
                }
                self.should_close = true;
            }
            Ok(FlowOutcome::Failure(reason)) => {
                self.error = Some(reason);
                self.password.clear();
                self.awaiting_response = false;
                self.focus = FocusedField::Password;
                if let Some(g) = self.greetd.as_mut() {
                    let _ = g.cancel();
                }
                self.pending = false;
            }
            Ok(FlowOutcome::Cancelled) => { self.pending = false; }
            Err(e) => {
                self.error = Some(format!("greetd protocol error: {e}"));
                self.pending = false;
            }
        }
        self.redraw(qh);
    }

    fn handle_action(&mut self, action: Action, qh: &QueueHandle<Self>) {
        match action {
            Action::Tab => {
                self.focus = match self.focus {
                    FocusedField::Username => FocusedField::Password,
                    FocusedField::Password => FocusedField::Username,
                };
            }
            Action::Backspace => {
                match self.focus {
                    FocusedField::Username => { self.username.pop(); }
                    FocusedField::Password => { self.password.pop(); }
                }
            }
            Action::Enter => {
                if matches!(self.focus, FocusedField::Username) && !self.awaiting_response {
                    self.focus = FocusedField::Password;
                } else {
                    self.submit(qh);
                    return;
                }
            }
            Action::Escape => {
                self.username.clear();
                self.password.clear();
                self.awaiting_response = false;
                self.error = None;
                if let Some(g) = self.greetd.as_mut() {
                    let _ = g.cancel();
                }
            }
            _ => {}
        }
        self.redraw(qh);
    }

    fn handle_text(&mut self, text: &str, qh: &QueueHandle<Self>) {
        if self.pending { return; }
        match self.focus {
            FocusedField::Username => self.username.push_str(text),
            FocusedField::Password => self.password.push_str(text),
        }
        self.redraw(qh);
    }
}

// ── Dispatch impls ──────────────────────────────────────────────────────────

impl Dispatch<XdgWmBase, ()> for GreetState {
    fn event(_: &mut Self, base: &XdgWmBase, event: xdg_wm_base::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping { serial } = event { base.pong(serial); }
    }
}
impl Dispatch<XdgSurface, ()> for GreetState {
    fn event(state: &mut Self, surf: &XdgSurface, event: xdg_surface::Event,
        _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let xdg_surface::Event::Configure { serial } = event {
            surf.ack_configure(serial);
            state.configured = true;
            state.redraw(qh);
        }
    }
}
impl Dispatch<XdgToplevel, ()> for GreetState {
    fn event(state: &mut Self, _: &XdgToplevel, event: xdg_toplevel::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if width  > 0 { state.width  = width  as u32; }
                if height > 0 { state.height = height as u32; }
            }
            xdg_toplevel::Event::Close => state.should_close = true,
            _ => {}
        }
    }
}
impl Dispatch<WlSeat, ()> for GreetState {
    fn event(state: &mut Self, seat: &WlSeat, event: wl_seat::Event,
        _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(caps) } = event {
            if caps.contains(wayland_client::protocol::wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}
impl Dispatch<WlKeyboard, ()> for GreetState {
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
                            KeySymbol::Text(t)   => state.handle_text(&t, qh),
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
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for GreetState {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents,
        _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlCompositor, ()> for GreetState { fn event(_: &mut Self, _: &WlCompositor, _: wayland_client::protocol::wl_compositor::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {} }
impl Dispatch<WlShm, ()> for GreetState { fn event(_: &mut Self, _: &WlShm, _: wayland_client::protocol::wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {} }
impl Dispatch<WlSurface, ()> for GreetState { fn event(_: &mut Self, _: &WlSurface, _: wayland_client::protocol::wl_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {} }
impl Dispatch<WlShmPool, ()> for GreetState { fn event(_: &mut Self, _: &WlShmPool, _: wayland_client::protocol::wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {} }
impl Dispatch<WlBuffer, BufferUserData> for GreetState {
    fn event(_: &mut Self, _: &WlBuffer,
        event: wayland_client::protocol::wl_buffer::Event,
        data: &BufferUserData, _: &Connection, _: &QueueHandle<Self>) {
        if let wayland_client::protocol::wl_buffer::Event::Release = event { data.mark_released(); }
    }
}

pub fn run() -> Result<()> {
    let conn = Connection::connect_to_env().context("no Wayland compositor")?;
    let (globals, mut queue) = registry_queue_init::<GreetState>(&conn)?;
    let qh = queue.handle();

    let compositor: WlCompositor = globals.bind(&qh, 4..=6, ())?;
    let shm:        WlShm        = globals.bind(&qh, 1..=1, ())?;
    let wm_base:    XdgWmBase    = globals.bind(&qh, 1..=6, ())?;
    let _seat:      WlSeat       = globals.bind(&qh, 5..=8, ())?;

    let mut state = GreetState::new(compositor, shm, wm_base);
    state.init_surface(&qh);

    while !state.should_close {
        queue.blocking_dispatch(&mut state)?;
    }
    Ok(())
}
