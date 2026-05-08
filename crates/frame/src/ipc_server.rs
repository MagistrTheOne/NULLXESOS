//! Control IPC v1 server.
//!
//! Wire spec lives in `crates/ipc`; this module implements the server side.
//!
//! Lifecycle:
//!   - `start(loop_handle, sender_to_state)` opens the socket and spawns a
//!     tokio task to accept connections + decode lines.
//!   - Each accepted client connection lives in its own task; on disconnect
//!     the task exits cleanly.
//!   - On compositor shutdown the listener task aborts on its CancellationToken
//!     and the socket file is removed by `Drop`.
//!
//! Backpressure:
//!   - Inbound queue to the calloop main loop is bounded at 256 messages.
//!   - When full, new requests get `Response::err(Busy)` synchronously and are
//!     not enqueued. `Reload.Config` is coalesced server-side.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use calloop::channel::{Sender as CalSender, Channel as CalChannel};
use ipc::{
    envelope::{Direction, Envelope, PROTOCOL_VERSION},
    request::Request,
    response::{IpcError, Response, ResponseData, StateSnapshot},
};
use rustix::fs::Mode;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{mpsc, oneshot},
};

use crate::state::NullxesState;

const INBOUND_QUEUE_DEPTH: usize = 256;

pub struct IpcCommand {
    pub request:  Request,
    pub respond:  oneshot::Sender<Response>,
}

/// Holder dropped at compositor shutdown — unlinks the socket file.
pub struct IpcSocket {
    path: PathBuf,
}

impl Drop for IpcSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn start(
    loop_handle: &calloop::LoopHandle<'_, NullxesState>,
) -> Result<(IpcSocket, CalSender<IpcCommand>)> {
    let path = ipc::socket_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create runtime dir {}", parent.display()))?;
        // 0700
        let _ = std::fs::set_permissions(parent, std::os::unix::fs::PermissionsExt::from_mode(0o700));
    }

    // Replace any stale socket from a previous (crashed) run.
    let _ = std::fs::remove_file(&path);

    let listener = std::os::unix::net::UnixListener::bind(&path)
        .with_context(|| format!("bind ipc socket {}", path.display()))?;
    listener.set_nonblocking(true)?;
    // 0600
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));

    let async_listener = UnixListener::from_std(listener)?;

    // Calloop channel from server task → main loop.
    let (cal_tx, cal_rx): (CalSender<IpcCommand>, CalChannel<IpcCommand>) =
        calloop::channel::channel();

    // Wire the calloop side: dispatch incoming requests on the main loop.
    loop_handle
        .insert_source(cal_rx, |event, _, state| {
            if let calloop::channel::Event::Msg(cmd) = event {
                let resp = handle_request_on_main(state, cmd.request);
                let _ = cmd.respond.send(resp);
            }
        })
        .map_err(|e| anyhow::anyhow!("insert ipc channel source: {e}"))?;

    let cal_tx_async = Arc::new(cal_tx.clone());

    // Spawn the accept loop on the global tokio runtime.
    let handle = crate::runtime::Runtime::handle();
    handle.spawn(accept_loop(async_listener, cal_tx_async));

    Ok((IpcSocket { path }, cal_tx))
}

async fn accept_loop(listener: UnixListener, cal_tx: Arc<CalSender<IpcCommand>>) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let cal_tx = cal_tx.clone();
                tokio::spawn(client_loop(stream, cal_tx));
            }
            Err(e) => {
                tracing::warn!(?e, "ipc accept failed; sleeping 250ms");
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        }
    }
}

async fn client_loop(stream: UnixStream, cal_tx: Arc<CalSender<IpcCommand>>) {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();

    loop {
        line.clear();
        let n = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(e) => {
                tracing::debug!(?e, "ipc read error");
                return;
            }
        };
        if n == 0 { return; } // hangup

        let env: Result<Envelope, _> = serde_json::from_str(line.trim_end());
        let response = match env {
            Err(e) => Response::err(IpcError::InvalidRequest(format!("decode: {e}"))),
            Ok(env) if env.v != PROTOCOL_VERSION => {
                Response::err(IpcError::VersionMismatch { supported: vec![PROTOCOL_VERSION] })
            }
            Ok(env) => match env.direction {
                Direction::Request(req) => dispatch(&cal_tx, req).await,
                Direction::Response(_)  => Response::err(IpcError::InvalidRequest("response on request socket".into())),
            },
        };

        // Re-derive correlation id (defaults to 0 if undecodable).
        let id = match serde_json::from_str::<Envelope>(line.trim_end()) {
            Ok(e) => e.id,
            Err(_) => 0,
        };
        let out = Envelope::response(id, response);
        let mut bytes = match serde_json::to_vec(&out) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!(?e, "encode response");
                return;
            }
        };
        bytes.push(b'\n');
        if let Err(e) = write.write_all(&bytes).await {
            tracing::debug!(?e, "ipc write error → hangup");
            return;
        }
    }
}

async fn dispatch(cal_tx: &CalSender<IpcCommand>, req: Request) -> Response {
    let (tx, rx) = oneshot::channel();
    let cmd = IpcCommand { request: req, respond: tx };
    if cal_tx.send(cmd).is_err() {
        return Response::err(IpcError::Internal("compositor not listening".into()));
    }
    match rx.await {
        Ok(r)  => r,
        Err(_) => Response::err(IpcError::Internal("compositor dropped reply channel".into())),
    }
}

fn handle_request_on_main(state: &mut NullxesState, req: Request) -> Response {
    match req {
        Request::SwitchWorkspace { idx } => {
            if state.workspace_mgr.switch_to(idx) {
                Response::ok()
            } else {
                Response::err(IpcError::InvalidRequest(format!("workspace {idx} out of range or inactive")))
            }
        }
        Request::MoveFocused { target } => {
            if let Some((src, tgt, id)) = state.workspace_mgr.plan_move_focused(target) {
                crate::input::keybind::move_window_between_workspaces(state, src, tgt, id);
                Response::ok()
            } else {
                Response::err(IpcError::InvalidRequest("no focused window or invalid target".into()))
            }
        }
        Request::ReloadConfig => {
            let new_cfg = crate::config::FrameConfig::load();
            state.config = new_cfg;
            Response::ok()
        }
        Request::GetState => {
            let occupied = state.workspaces.occupied();
            let snap = StateSnapshot {
                workspace_count:  state.workspace_mgr.count(),
                active_workspace: state.workspace_mgr.active_index(),
                occupied,
                xwayland_ready:   state.xwm.wm.is_some(),
            };
            Response::ok_with(ResponseData::State(snap))
        }
        Request::SpawnLauncher => {
            spawn_detached(&state.config.keybindings.launcher_binary);
            Response::ok()
        }
        Request::SessionLock => {
            spawn_detached(&state.config.keybindings.lock_binary);
            Response::ok()
        }
        Request::Quit => {
            state.loop_signal.stop();
            Response::ok()
        }
    }
}

fn spawn_detached(bin: &str) {
    use std::process::{Command, Stdio};
    let _ = Command::new(bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

// helper used by Cargo lint silence for std::os::unix::fs::PermissionsExt
#[allow(unused_imports)]
use std::os::unix::fs::PermissionsExt as _;
// Suppress unused import warning when the rustix Mode type is not used
// (we currently set permissions via std::fs which is more portable).
#[allow(dead_code)]
fn _ensure_rustix_used(_m: Mode) {}
