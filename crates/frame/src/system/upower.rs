//! UPower D-Bus listener — fills `ModuleSnapshot.battery_pct` + `on_ac`.
//!
//! Subscribes to `org.freedesktop.UPower` `DeviceAdded` / `DeviceRemoved` and
//! per-device `PropertiesChanged`. We watch only the first device of type
//! `Battery` (per UPower convention `Type=2`).

use std::sync::Arc;

use parking_lot::Mutex;
use zbus::Connection;
use zbus::zvariant::OwnedObjectPath;
use futures_util::StreamExt;

use crate::panel::ModuleSnapshot;

pub async fn run(snapshot: Arc<Mutex<ModuleSnapshot>>) -> anyhow::Result<()> {
    let conn = Connection::system().await?;
    let proxy = upower_zbus::UPowerProxy::new(&conn).await?;
    let mut on_ac = proxy.on_battery().await.map(|b| !b).unwrap_or(true);
    snapshot.lock().on_ac = on_ac;

    let devices = proxy.enumerate_devices().await?;
    let battery_path = pick_battery(&conn, &devices).await;
    if let Some(path) = battery_path {
        let dev = upower_zbus::DeviceProxy::builder(&conn)
            .path(path.clone())?
            .build()
            .await?;
        let pct = dev.percentage().await.unwrap_or(0.0).round().clamp(0.0, 100.0) as u8;
        snapshot.lock().battery_pct = Some(pct);

        // Watch property changes — hot loop.
        let mut props_changed = dev.receive_percentage_changed().await;
        let mut on_battery = proxy.receive_on_battery_changed().await;
        loop {
            tokio::select! {
                Some(change) = props_changed.next() => {
                    if let Ok(v) = change.get().await {
                        snapshot.lock().battery_pct = Some(v.round().clamp(0.0, 100.0) as u8);
                    }
                }
                Some(ac) = on_battery.next() => {
                    if let Ok(b) = ac.get().await {
                        on_ac = !b;
                        snapshot.lock().on_ac = on_ac;
                    }
                }
                else => { break; }
            }
        }
    }
    Ok(())
}

async fn pick_battery(conn: &Connection, paths: &[OwnedObjectPath]) -> Option<OwnedObjectPath> {
    for path in paths {
        let Ok(p) = upower_zbus::DeviceProxy::builder(conn).path(path.clone()).ok()?.build().await else { continue; };
        if let Ok(kind) = p.type_().await {
            // 2 == battery
            if kind == 2 { return Some(path.clone()); }
        }
    }
    None
}

mod upower_zbus {
    //! Hand-rolled minimal proxy (no `zbus_proxy!` macro to keep deps light).

    use zbus::{proxy, Result};

    #[proxy(
        interface = "org.freedesktop.UPower",
        default_service = "org.freedesktop.UPower",
        default_path    = "/org/freedesktop/UPower"
    )]
    pub trait UPower {
        #[zbus(property)]
        fn on_battery(&self) -> Result<bool>;
        fn enumerate_devices(&self) -> Result<Vec<zbus::zvariant::OwnedObjectPath>>;
    }

    #[proxy(
        interface = "org.freedesktop.UPower.Device",
        default_service = "org.freedesktop.UPower"
    )]
    pub trait Device {
        #[zbus(property)]
        fn percentage(&self) -> Result<f64>;
        #[zbus(property, name = "Type")]
        fn type_(&self) -> Result<u32>;
    }
}
