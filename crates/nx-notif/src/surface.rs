//! Wayland thread + layer-shell toast surface.
//!
//! Maintains a queue of notifications. Renders up to `MAX_VISIBLE` of them
//! stacked top-to-bottom in the top-right corner. Each toast lives for
//! `notif.timeout_ms` then fires `tx_closed` (broadcast) so the D-Bus side
//! emits `NotificationClosed`.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
        wl_registry,
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use wlcommon::{shm::{BufferUserData, ShmDoubleBuffer}, SurfaceKind};

use crate::render;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urgency { Low, Normal, Critical }

#[derive(Clone, Debug)]
pub struct Notif {
    pub id:         u32,
    pub app_name:   String,
    pub app_icon:   String,
    pub summary:    String,
    pub body:       String,
    pub actions:    Vec<String>,
    pub urgency:    Urgency,
    /// 0 (sticky) or ms.
    pub timeout_ms: u32,
}

impl Notif {
    pub fn close_for(id: u32) -> Self {
        Self {
            id,
            app_name: String::new(),
            app_icon: String::new(),
            summary:  String::new(),
            body:     String::new(),
            actions:  Vec::new(),
            urgency:  Urgency::Low,
            timeout_ms: 1, // immediate close
        }
    }
}

const MAX_VISIBLE: usize = 5;

struct VisibleToast {
    notif:        Notif,
    surface:      WlSurface,
    layer:        ZwlrLayerSurfaceV1,
    buffer:       Option<ShmDoubleBuffer<WaylandThreadState>>,
    configured:   bool,
    width:        u32,
    height:       u32,
    expires_at:   Instant,
}

pub struct WaylandThreadState {
    compositor:  WlCompositor,
    shm:         WlShm,
    layer_shell: ZwlrLayerShellV1,
    text:        Option<theme::text::TextRenderer>,
    visible:     Vec<VisibleToast>,
    queue:       VecDeque<Notif>,
    tx_closed:   tokio::sync::broadcast::Sender<u32>,
    quit:        bool,
}

impl SurfaceKind for WaylandThreadState {}

impl WaylandThreadState {
    fn enqueue(&mut self, notif: Notif, qh: &QueueHandle<Self>) {
        // If this notif id is already visible, replace.
        if let Some(slot) = self.visible.iter_mut().find(|v| v.notif.id == notif.id) {
            slot.notif = notif;
            slot.expires_at = Instant::now() + Duration::from_millis(slot.notif.timeout_ms.max(1) as u64);
            // Force redraw.
            slot.buffer = None;
            return;
        }
        self.queue.push_back(notif);
        self.pump_visible(qh);
    }

    fn pump_visible(&mut self, qh: &QueueHandle<Self>) {
        while self.visible.len() < MAX_VISIBLE {
            let Some(n) = self.queue.pop_front() else { break; };
            let surface = self.compositor.create_surface(qh, ());
            let layer = self.layer_shell.get_layer_surface(
                &surface, None, Layer::Overlay,
                "nullxes-notif".to_string(), qh, (),
            );
            layer.set_size(render::TOAST_W, render::TOAST_H);
            layer.set_anchor(Anchor::Top | Anchor::Right);
            layer.set_margin(
                16 + ((self.visible.len() as i32) * (render::TOAST_H as i32 + 12)),
                16, 0, 0,
            );
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            surface.commit();
            let expires = Instant::now() + Duration::from_millis(n.timeout_ms.max(1) as u64);
            self.visible.push(VisibleToast {
                notif: n,
                surface, layer,
                buffer: None, configured: false,
                width: render::TOAST_W, height: render::TOAST_H,
                expires_at: expires,
            });
        }
    }

    fn redraw(&mut self, idx: usize, qh: &QueueHandle<Self>) {
        let Some(t) = self.visible.get_mut(idx) else { return; };
        if !t.configured { return; }
        if t.buffer.is_none() {
            match ShmDoubleBuffer::new(self.shm.clone(), qh.clone(), t.width, t.height) {
                Ok(b) => t.buffer = Some(b),
                Err(e) => { tracing::error!(?e, "shm alloc"); return; }
            }
        }
        let Some(pool) = t.buffer.as_mut() else { return; };
        let notif = t.notif.clone();
        let mut text_taken = self.text.take();
        let drawn = match pool.draw(|pixels, stride, w, h| {
            render::paint(pixels, stride, w, h, text_taken.as_mut(), &notif);
        }) {
            Ok(d) => d,
            Err(e) => { tracing::warn!(?e, "redraw"); self.text = text_taken; return; }
        };
        drawn.attach_and_commit(&t.surface);
        self.text = text_taken;
    }

    fn tick_timeouts(&mut self) {
        let now = Instant::now();
        let mut closed: Vec<u32> = Vec::new();
        self.visible.retain(|t| {
            if now >= t.expires_at {
                closed.push(t.notif.id);
                false
            } else { true }
        });
        for id in closed {
            let _ = self.tx_closed.send(id);
        }
    }
}

// ── Dispatch impls ──────────────────────────────────────────────────────────

impl Dispatch<ZwlrLayerSurfaceV1, ()> for WaylandThreadState {
    fn event(state: &mut Self, ls: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event, _: &(),
        _: &Connection, qh: &QueueHandle<Self>) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                ls.ack_configure(serial);
                let pos = state.visible.iter().position(|t| &t.layer == ls);
                if let Some(idx) = pos {
                    if let Some(t) = state.visible.get_mut(idx) {
                        t.configured = true;
                    }
                    state.redraw(idx, qh);
                }
            }
            zwlr_layer_surface_v1::Event::Closed => {
                let pos = state.visible.iter().position(|t| &t.layer == ls);
                if let Some(idx) = pos {
                    let id = state.visible[idx].notif.id;
                    state.visible.remove(idx);
                    let _ = state.tx_closed.send(id);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for WaylandThreadState {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry,
        _: wl_registry::Event, _: &GlobalListContents,
        _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlCompositor, ()> for WaylandThreadState {
    fn event(_: &mut Self, _: &WlCompositor,
        _: wayland_client::protocol::wl_compositor::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShm, ()> for WaylandThreadState {
    fn event(_: &mut Self, _: &WlShm,
        _: wayland_client::protocol::wl_shm::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwlrLayerShellV1, ()> for WaylandThreadState {
    fn event(_: &mut Self, _: &ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlSurface, ()> for WaylandThreadState {
    fn event(_: &mut Self, _: &WlSurface,
        _: wayland_client::protocol::wl_surface::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShmPool, ()> for WaylandThreadState {
    fn event(_: &mut Self, _: &WlShmPool,
        _: wayland_client::protocol::wl_shm_pool::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlBuffer, BufferUserData> for WaylandThreadState {
    fn event(_: &mut Self, _: &WlBuffer,
        event: wayland_client::protocol::wl_buffer::Event,
        data: &BufferUserData, _: &Connection, _: &QueueHandle<Self>) {
        if let wayland_client::protocol::wl_buffer::Event::Release = event {
            data.mark_released();
        }
    }
}

pub fn wayland_thread(
    mut rx: tokio::sync::mpsc::Receiver<Notif>,
    tx_closed: tokio::sync::broadcast::Sender<u32>,
) -> anyhow::Result<()> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<WaylandThreadState>(&conn)?;
    let qh = queue.handle();

    let compositor:  WlCompositor    = globals.bind(&qh, 4..=6, ())?;
    let shm:         WlShm           = globals.bind(&qh, 1..=1, ())?;
    let layer_shell: ZwlrLayerShellV1 = globals.bind(&qh, 1..=4, ())?;

    let mut state = WaylandThreadState {
        compositor, shm, layer_shell,
        text: wlcommon::load_default_text_renderer(),
        visible: Vec::new(),
        queue: VecDeque::new(),
        tx_closed,
        quit: false,
    };

    let poll_interval = std::time::Duration::from_millis(50);
    while !state.quit {
        // Drain any queued notifications.
        while let Ok(notif) = rx.try_recv() {
            // close_for sentinel: timeout==1 and empty summary → just expire.
            if notif.summary.is_empty() && notif.body.is_empty() && notif.timeout_ms <= 1 {
                if let Some(idx) = state.visible.iter().position(|t| t.notif.id == notif.id) {
                    state.visible.remove(idx);
                    let _ = state.tx_closed.send(notif.id);
                }
                continue;
            }
            state.enqueue(notif, &qh);
        }

        // Dispatch any pending wayland events without blocking.
        if let Err(e) = queue.dispatch_pending(&mut state) {
            tracing::error!(?e, "wayland dispatch");
            break;
        }
        // Flush + read events with timeout via blocking_dispatch on a short tick:
        // We use roundtrip+poll_pending semantics by sleeping.
        std::thread::sleep(poll_interval);
        state.tick_timeouts();
    }
    Ok(())
}
