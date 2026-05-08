//! org.freedesktop.Notifications D-Bus interface.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use zbus::{conn::Builder, interface, ConnectionBuilder, Connection, SignalContext};
use zvariant::{OwnedValue, Value};

use crate::history::History;
use crate::surface::{Notif, Urgency};

const BUS_NAME:      &str = "org.freedesktop.Notifications";
const OBJECT_PATH:   &str = "/org/freedesktop/Notifications";
const IFACE_NAME:    &str = "org.freedesktop.Notifications";

/// Stable across notify() calls within a single session, ascending.
type NotifId = u32;

pub struct Server {
    inner: Arc<ServerInner>,
}

struct ServerInner {
    next_id:     parking_lot::Mutex<NotifId>,
    tx_to_wl:    tokio::sync::mpsc::Sender<Notif>,
    rx_closed:   tokio::sync::Mutex<tokio::sync::broadcast::Receiver<u32>>,
    history:     Mutex<History>,
}

impl Server {
    pub fn new(
        tx_to_wl:  tokio::sync::mpsc::Sender<Notif>,
        rx_closed: tokio::sync::broadcast::Receiver<u32>,
        history:   History,
    ) -> Self {
        Self {
            inner: Arc::new(ServerInner {
                next_id: parking_lot::Mutex::new(1),
                tx_to_wl,
                rx_closed: tokio::sync::Mutex::new(rx_closed),
                history: Mutex::new(history),
            }),
        }
    }

    pub async fn start(self) -> Result<Connection> {
        let inner = self.inner.clone();
        let conn = Builder::session()?
            .name(BUS_NAME)?
            .serve_at(OBJECT_PATH, NotificationsService { inner: inner.clone() })?
            .build()
            .await?;
        tracing::info!(bus = %BUS_NAME, "claimed bus name");

        // Spawn relay: wl thread → bus signal NotificationClosed.
        let inner2 = inner.clone();
        let conn2 = conn.clone();
        tokio::spawn(async move {
            loop {
                let mut rx = inner2.rx_closed.lock().await;
                match rx.recv().await {
                    Ok(id) => {
                        if let Err(e) = signal_closed(&conn2, id, 1).await {
                            tracing::warn!(?e, "emit NotificationClosed");
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(_) => continue,
                }
            }
        });

        Ok(conn)
    }
}

async fn signal_closed(conn: &Connection, id: u32, reason: u32) -> zbus::Result<()> {
    let ctx = SignalContext::new(conn, OBJECT_PATH)?;
    NotificationsService::notification_closed(&ctx, id, reason).await
}

pub struct NotificationsService {
    inner: Arc<ServerInner>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationsService {
    /// `Notify` per Desktop Notifications Spec 1.2.
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name:  String,
        replaces_id: u32,
        app_icon:  String,
        summary:   String,
        body:      String,
        actions:   Vec<String>,
        hints:     HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let mut id_slot = self.inner.next_id.lock();
        let id = if replaces_id != 0 { replaces_id } else {
            let v = *id_slot; *id_slot = id_slot.wrapping_add(1).max(1); v
        };
        drop(id_slot);

        let urgency = hints.get("urgency").and_then(|v| {
            // urgency is byte (0/1/2)
            match v.downcast_ref::<u8>() {
                Some(0) => Some(Urgency::Low),
                Some(1) => Some(Urgency::Normal),
                Some(2) => Some(Urgency::Critical),
                _       => None,
            }
        }).unwrap_or(Urgency::Normal);

        let notif = Notif {
            id,
            app_name: app_name.clone(),
            app_icon: app_icon.clone(),
            summary: summary.clone(),
            body: body.clone(),
            actions: actions.clone(),
            urgency,
            timeout_ms: if expire_timeout < 0 { 5_000 } else { expire_timeout as u32 },
        };

        // Persist history (best-effort).
        {
            let mut h = self.inner.history.lock();
            if let Err(e) = h.append(&notif) {
                tracing::warn!(?e, "history append");
            }
        }

        // Deliver to wayland thread.
        if let Err(e) = self.inner.tx_to_wl.send(notif).await {
            tracing::warn!(?e, "wayland thread mpsc full / dropped");
        }
        id
    }

    async fn close_notification(&self, id: u32) {
        // Forward as a special "close" message — wayland thread treats id with
        // duplicate insertion of the same id as a hide trigger.
        let _ = self.inner.tx_to_wl.send(Notif::close_for(id)).await;
    }

    async fn get_capabilities(&self) -> Vec<String> {
        vec![
            "body".into(),
            "body-markup".into(),
            "actions".into(),
            "icon-static".into(),
            "persistence".into(),
        ]
    }

    async fn get_server_information(&self) -> (String, String, String, String) {
        (
            "NX-NOTIF".into(),
            "NULLXES".into(),
            env!("CARGO_PKG_VERSION").into(),
            "1.2".into(),
        )
    }

    /// Emitted when a notification is closed — reason: 1=expired, 2=dismissed,
    /// 3=closed by call, 4=undefined.
    #[zbus(signal)]
    async fn notification_closed(ctx: &SignalContext<'_>, id: u32, reason: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(ctx: &SignalContext<'_>, id: u32, action_key: String) -> zbus::Result<()>;
}
