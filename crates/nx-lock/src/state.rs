//! Wayland + ext-session-lock-v1 client logic.

use std::sync::mpsc;

use anyhow::{Context, Result};
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_keyboard::{self, WlKeyboard},
        wl_output::{self, WlOutput},
        wl_seat::{self, WlSeat},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
        wl_registry,
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1::{self, ExtSessionLockManagerV1},
    ext_session_lock_surface_v1::{self, ExtSessionLockSurfaceV1},
    ext_session_lock_v1::{self, ExtSessionLockV1},
};

use wlcommon::{
    keymap::{Action, KeySymbol, KeyboardState},
    shm::{BufferUserData, ShmDoubleBuffer},
    SurfaceKind,
};

use crate::{pam_auth::{AuthResult, AuthWorker, current_username}, render};

struct LockOutput {
    output:       WlOutput,
    width:        u32,
    height:       u32,
    surface:      Option<WlSurface>,
    lock_surface: Option<ExtSessionLockSurfaceV1>,
    buffer:       Option<ShmDoubleBuffer<LockState>>,
    configured:   bool,
}

pub struct LockState {
    shm:           WlShm,
    compositor:    WlCompositor,
    lock_manager:  ExtSessionLockManagerV1,
    lock:          Option<ExtSessionLockV1>,
    locked:        bool,
    outputs:       Vec<LockOutput>,
    keyboard:      Option<WlKeyboard>,
    keymap:        KeyboardState,
    text:          Option<theme::text::TextRenderer>,
    username:      String,
    password:      String,
    fail_count:    u32,
    pending:       bool,
    auth:          AuthWorker,
    auth_rx:       mpsc::Receiver<AuthResult>,
    auth_tx:       mpsc::Sender<AuthResult>,
    pub should_quit: bool,
    pub exit_code: i32,
}

impl SurfaceKind for LockState {}

impl LockState {
    fn new(
        shm:          WlShm,
        compositor:   WlCompositor,
        lock_manager: ExtSessionLockManagerV1,
        service:      &'static str,
    ) -> Self {
        let (auth_tx, auth_rx) = mpsc::channel();
        Self {
            shm, compositor, lock_manager,
            lock: None,
            locked: false,
            outputs: Vec::new(),
            keyboard: None,
            keymap: KeyboardState::new(),
            text: wlcommon::load_default_text_renderer(),
            username: current_username(),
            password: String::new(),
            fail_count: 0,
            pending: false,
            auth: AuthWorker::spawn(service),
            auth_rx,
            auth_tx,
            should_quit: false,
            exit_code: 0,
        }
    }

    fn engage(&mut self, qh: &QueueHandle<Self>) {
        let lock = self.lock_manager.lock(qh, ());
        self.lock = Some(lock);
    }

    fn poll_auth(&mut self, qh: &QueueHandle<Self>) {
        while let Ok(result) = self.auth_rx.try_recv() {
            self.pending = false;
            match result {
                AuthResult::Ok => {
                    tracing::info!("PAM ok → unlocking");
                    if let Some(lock) = self.lock.as_ref() {
                        lock.unlock_and_destroy();
                    }
                    self.should_quit = true;
                    self.exit_code = 0;
                }
                AuthResult::Fail(reason) => {
                    self.fail_count += 1;
                    tracing::warn!(%reason, count = self.fail_count, "PAM failed");
                    self.password.clear();
                    self.repaint_all(qh);
                    // Brute-force throttle.
                    std::thread::sleep(std::time::Duration::from_millis(120));
                }
            }
        }
    }

    fn submit_password(&mut self, qh: &QueueHandle<Self>) {
        if self.password.is_empty() || self.pending { return; }
        self.pending = true;
        self.repaint_all(qh);
        let _ = self.auth.tx.send(crate::pam_auth::AuthRequest {
            username: self.username.clone(),
            password: std::mem::take(&mut self.password),
            respond:  self.auth_tx.clone(),
        });
    }

    fn repaint_all(&mut self, qh: &QueueHandle<Self>) {
        // Borrow each output mutably without aliasing self by indexing.
        for i in 0..self.outputs.len() {
            self.repaint_output(i, qh);
        }
    }

    fn repaint_output(&mut self, idx: usize, qh: &QueueHandle<Self>) {
        // Capture immutable bits we need, then mutably borrow the output.
        let username = self.username.clone();
        let password_len = self.password.len();
        let fail_count = self.fail_count;
        let pending = self.pending;
        let mut text_taken = self.text.take();

        let Some(out) = self.outputs.get_mut(idx) else { self.text = text_taken; return; };
        if !out.configured { self.text = text_taken; return; }
        if out.buffer.is_none() {
            match ShmDoubleBuffer::new(self.shm.clone(), qh.clone(), out.width, out.height) {
                Ok(b) => out.buffer = Some(b),
                Err(e) => { tracing::error!(?e, "lock shm alloc"); self.text = text_taken; return; }
            }
        }
        let Some(pool) = out.buffer.as_mut() else { self.text = text_taken; return; };

        let drawn = match pool.draw(|pixels, stride, w, h| {
            render::paint(pixels, stride, w, h, text_taken.as_mut(),
                &username, password_len, fail_count, pending);
        }) {
            Ok(d) => d,
            Err(e) => { tracing::warn!(?e, "lock redraw skip"); self.text = text_taken; return; }
        };
        let Some(surface) = out.surface.as_ref() else { self.text = text_taken; return; };
        drawn.attach_and_commit(surface);
        self.text = text_taken;
    }

    fn handle_action(&mut self, action: Action, qh: &QueueHandle<Self>) {
        match action {
            Action::Enter     => self.submit_password(qh),
            Action::Backspace => { self.password.pop(); self.repaint_all(qh); }
            Action::Escape    => { self.password.clear(); self.repaint_all(qh); }
            _ => {}
        }
    }

    fn handle_text(&mut self, text: &str, qh: &QueueHandle<Self>) {
        if !self.pending {
            self.password.push_str(text);
            self.repaint_all(qh);
        }
    }
}

// ── Dispatch ────────────────────────────────────────────────────────────────

impl Dispatch<ExtSessionLockManagerV1, ()> for LockState {
    fn event(_: &mut Self, _: &ExtSessionLockManagerV1,
        _: ext_session_lock_manager_v1::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ExtSessionLockV1, ()> for LockState {
    fn event(state: &mut Self, lock: &ExtSessionLockV1,
        event: ext_session_lock_v1::Event, _: &(),
        _: &Connection, qh: &QueueHandle<Self>) {
        match event {
            ext_session_lock_v1::Event::Locked => {
                tracing::info!("compositor confirmed lock");
                state.locked = true;
                // Create lock surfaces for any outputs we already discovered.
                let outputs_len = state.outputs.len();
                for i in 0..outputs_len {
                    state.create_lock_surface_for(i, lock, qh);
                }
            }
            ext_session_lock_v1::Event::Finished => {
                tracing::warn!("compositor refused / dropped lock; exiting nonzero so systemd respawns us");
                state.should_quit = true;
                state.exit_code = 1;
            }
            _ => {}
        }
    }
}

impl LockState {
    fn create_lock_surface_for(&mut self, idx: usize, lock: &ExtSessionLockV1, qh: &QueueHandle<Self>) {
        let Some(out) = self.outputs.get_mut(idx) else { return; };
        if out.lock_surface.is_some() { return; }
        let surface = self.compositor.create_surface(qh, ());
        let lock_surface = lock.get_lock_surface(&surface, &out.output, qh, ());
        out.surface = Some(surface);
        out.lock_surface = Some(lock_surface);
    }
}

impl Dispatch<ExtSessionLockSurfaceV1, ()> for LockState {
    fn event(state: &mut Self, surf: &ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event, _: &(),
        _: &Connection, qh: &QueueHandle<Self>) {
        if let ext_session_lock_surface_v1::Event::Configure { serial, width, height } = event {
            surf.ack_configure(serial);
            // Find the output that owns this lock_surface.
            let pos = state.outputs.iter().position(|o| {
                o.lock_surface.as_ref() == Some(surf)
            });
            if let Some(idx) = pos {
                if let Some(o) = state.outputs.get_mut(idx) {
                    o.width  = width.max(1);
                    o.height = height.max(1);
                    o.configured = true;
                }
                state.repaint_output(idx, qh);
            }
        }
    }
}

impl Dispatch<WlOutput, ()> for LockState {
    fn event(state: &mut Self, output: &WlOutput,
        event: wl_output::Event, _: &(),
        _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_output::Event::Mode { width, height, .. } = event {
            // Find or insert.
            if let Some(o) = state.outputs.iter_mut().find(|o| &o.output == output) {
                o.width = width as u32;
                o.height = height as u32;
            } else {
                state.outputs.push(LockOutput {
                    output:       output.clone(),
                    width:        width as u32,
                    height:       height as u32,
                    surface:      None,
                    lock_surface: None,
                    buffer:       None,
                    configured:   false,
                });
            }
            // If lock already engaged, immediately create surface for this output.
            if state.locked {
                let last = state.outputs.len().saturating_sub(1);
                if let Some(lock) = state.lock.clone() {
                    state.create_lock_surface_for(last, &lock, qh);
                }
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for LockState {
    fn event(state: &mut Self, seat: &WlSeat, event: wl_seat::Event,
        _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(caps) } = event {
            if caps.contains(wayland_client::protocol::wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}

impl Dispatch<WlKeyboard, ()> for LockState {
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
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for LockState {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry,
        _: wl_registry::Event, _: &GlobalListContents,
        _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlCompositor, ()> for LockState {
    fn event(_: &mut Self, _: &WlCompositor,
        _: wayland_client::protocol::wl_compositor::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShm, ()> for LockState {
    fn event(_: &mut Self, _: &WlShm,
        _: wayland_client::protocol::wl_shm::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlSurface, ()> for LockState {
    fn event(_: &mut Self, _: &WlSurface,
        _: wayland_client::protocol::wl_surface::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShmPool, ()> for LockState {
    fn event(_: &mut Self, _: &WlShmPool,
        _: wayland_client::protocol::wl_shm_pool::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlBuffer, BufferUserData> for LockState {
    fn event(_: &mut Self, _: &WlBuffer,
        event: wayland_client::protocol::wl_buffer::Event,
        data: &BufferUserData, _: &Connection, _: &QueueHandle<Self>) {
        if let wayland_client::protocol::wl_buffer::Event::Release = event {
            data.mark_released();
        }
    }
}

pub fn run(service: &'static str) -> Result<i32> {
    let conn = Connection::connect_to_env().context("no Wayland compositor")?;
    let (globals, mut queue) = registry_queue_init::<LockState>(&conn)?;
    let qh = queue.handle();

    let compositor: WlCompositor = globals.bind(&qh, 4..=6, ())?;
    let shm:        WlShm        = globals.bind(&qh, 1..=1, ())?;
    let lock_mgr:   ExtSessionLockManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let _seat:      WlSeat       = globals.bind(&qh, 5..=8, ())?;

    // Outputs are bound by the registry; we listen for their events through
    // the registry queue. (Most compositors expose at least one output.)

    let mut state = LockState::new(shm, compositor, lock_mgr, service);
    state.engage(&qh);

    while !state.should_quit {
        queue.blocking_dispatch(&mut state)?;
        state.poll_auth(&qh);
    }
    Ok(state.exit_code)
}

// state::run returns i32 but main expects Result<()>; provide a tiny shim.
pub fn run_main(service: &'static str) -> Result<()> {
    let code = run(service)?;
    if code != 0 { std::process::exit(code); }
    Ok(())
}
