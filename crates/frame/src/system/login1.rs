//! systemd-logind D-Bus listener — `PrepareForSleep` for suspend/resume.
//!
//! Subscribes to `org.freedesktop.login1.Manager.PrepareForSleep`. On `true`
//! (going to sleep) we ask the compositor to spawn `nullxes-lock` if
//! `lock_on_suspend` is enabled, then flush surfaces. On `false` (waking)
//! we trigger a full repaint.
//!
//! Wired in Phase 2 once the calloop side has a non-blocking way to schedule
//! "redraw on next iteration"; the proxy and async logic are ready here.

use futures_util::StreamExt;
use zbus::Connection;

pub async fn run<F>(mut on_event: F) -> anyhow::Result<()>
where
    F: FnMut(LoginEvent) + Send + 'static,
{
    let conn = Connection::system().await?;
    let mgr = login1_zbus::ManagerProxy::new(&conn).await?;

    let mut prepare_sleep = mgr.receive_prepare_for_sleep().await?;
    while let Some(evt) = prepare_sleep.next().await {
        if let Ok(args) = evt.args() {
            on_event(if args.start { LoginEvent::Sleeping } else { LoginEvent::Wake });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum LoginEvent {
    Sleeping,
    Wake,
}

mod login1_zbus {
    use zbus::proxy;

    #[proxy(
        interface = "org.freedesktop.login1.Manager",
        default_service = "org.freedesktop.login1",
        default_path    = "/org/freedesktop/login1"
    )]
    pub trait Manager {
        #[zbus(signal)]
        fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
    }
}
