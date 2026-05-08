//! NetworkManager D-Bus listener — primary connection state.

use std::sync::Arc;

use futures_util::StreamExt;
use parking_lot::Mutex;
use zbus::Connection;

use crate::panel::{ModuleSnapshot, NetworkKind, NetworkSnapshot};

pub async fn run(snapshot: Arc<Mutex<ModuleSnapshot>>) -> anyhow::Result<()> {
    let conn = Connection::system().await?;
    let nm = nm_zbus::NetworkManagerProxy::new(&conn).await?;
    update_once(&conn, &nm, &snapshot).await;
    let mut state_changes = nm.receive_state_changed().await;
    while let Some(_evt) = state_changes.next().await {
        update_once(&conn, &nm, &snapshot).await;
    }
    Ok(())
}

async fn update_once(
    conn: &Connection,
    nm:   &nm_zbus::NetworkManagerProxy<'_>,
    snap: &Mutex<ModuleSnapshot>,
) {
    let connectivity = nm.state().await.unwrap_or(0);
    let connected = connectivity >= 70; // NM_STATE_CONNECTED_LOCAL = 50, _SITE = 60, _GLOBAL = 70
    let primary = nm.primary_connection().await.ok();

    let mut kind = NetworkKind::None;
    let mut ssid: Option<String> = None;

    if let Some(path) = primary {
        if !path.as_str().is_empty() && path.as_str() != "/" {
            // Read connection type via active-connection proxy.
            if let Ok(active) = nm_zbus::ActiveConnectionProxy::builder(conn)
                .path(path.clone()).map(|b| b.build())
                .map_err(|e| { tracing::debug!(?e, "active conn proxy"); e })
            {
                if let Ok(active) = active.await {
                    if let Ok(t) = active.connection_type().await {
                        kind = match t.as_str() {
                            "802-3-ethernet" => NetworkKind::Wired,
                            "802-11-wireless" => NetworkKind::Wireless,
                            _ => NetworkKind::Other,
                        };
                    }
                    if kind == NetworkKind::Wireless {
                        if let Ok(id) = active.id().await {
                            ssid = Some(id);
                        }
                    }
                }
            }
        }
    }

    let mut g = snap.lock();
    g.network = NetworkSnapshot { kind, connected, ssid, strength: None };
}

mod nm_zbus {
    use zbus::{proxy, Result, zvariant::OwnedObjectPath};

    #[proxy(
        interface = "org.freedesktop.NetworkManager",
        default_service = "org.freedesktop.NetworkManager",
        default_path    = "/org/freedesktop/NetworkManager"
    )]
    pub trait NetworkManager {
        #[zbus(property)]
        fn state(&self) -> Result<u32>;
        #[zbus(property)]
        fn primary_connection(&self) -> Result<OwnedObjectPath>;
    }

    #[proxy(
        interface = "org.freedesktop.NetworkManager.Connection.Active",
        default_service = "org.freedesktop.NetworkManager"
    )]
    pub trait ActiveConnection {
        #[zbus(property, name = "Type")]
        fn connection_type(&self) -> Result<String>;
        #[zbus(property)]
        fn id(&self) -> Result<String>;
    }
}
