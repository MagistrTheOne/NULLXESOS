//! Development backend: opens a winit window and serves wayland clients
//! through it. This is the fastest path to "Super opens a real launcher in a
//! real wayland session" on a developer's existing desktop.

use anyhow::{Context, Result};
use smithay::{
    backend::winit::{self, WinitEvent},
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, Mode as CalMode, PostAction},
        wayland_server::{Display, DisplayHandle},
    },
};
use tracing::info;

use crate::{
    config::FrameConfig,
    state::{BackendKind, NullxesState, PrimaryOutput},
};

pub fn run(config: FrameConfig) -> Result<()> {
    let mut event_loop: EventLoop<NullxesState> = EventLoop::try_new()
        .context("calloop EventLoop::try_new")?;
    let mut display: Display<NullxesState> = Display::new()
        .context("Display::new")?;
    let dh: DisplayHandle = display.handle();

    let socket_source = smithay::wayland::socket::ListeningSocketSource::new_auto()
        .context("create wayland listening socket")?;
    let socket_name = socket_source.socket_name().to_string_lossy().to_string();

    let (mut backend, winit_evl) = winit::init().context("winit::init")?;
    let mut state = NullxesState::new(
        event_loop.get_signal(),
        dh.clone(),
        config.clone(),
        BackendKind::Winit,
    );

    // Output bring-up.
    let size = backend.window_size();
    let output = Output::new(
        "winit".into(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "NULLXES".into(),
            model: "Dev".into(),
        },
    );
    output.create_global::<NullxesState>(&dh);
    let mode = Mode { size, refresh: 60_000 };
    output.change_current_state(Some(mode), None, None, Some((0, 0).into()));
    output.set_preferred(mode);
    for ws in 0..state.workspaces.count() {
        if let Some(space) = state.workspaces.get_mut(ws) {
            space.map_output(&output, (0, 0));
        }
    }
    state.primary_output = Some(PrimaryOutput { output: output.clone() });

    // Publish socket name.
    publish_wayland_display(&socket_name)?;
    info!(socket = %socket_name, "WAYLAND_DISPLAY published");

    // Insert wayland listening source: each accepted client is registered
    // with our `ClientState` userdata so handlers can find their per-client
    // CompositorClientState.
    let dh_for_clients = dh.clone();
    event_loop
        .handle()
        .insert_source(socket_source, move |stream, _, _state| {
            if let Err(e) = dh_for_clients
                .insert_client(stream, std::sync::Arc::new(crate::state::ClientState::default()))
            {
                tracing::warn!(?e, "failed to insert wayland client");
            }
        })
        .map_err(|e| anyhow::anyhow!("insert wayland socket source: {e}"))?;

    // Wayland display source (calloop reads from the wayland file descriptor).
    let dispatcher: Generic<Display<NullxesState>> = Generic::new(
        display,
        Interest::READ,
        CalMode::Level,
    );
    event_loop.handle().insert_source(dispatcher, |_, display, state| {
        // Safety: `display` is borrowed only inside this callback; smithay
        // requires us to dispatch and then flush.
        if let Err(e) = display.dispatch_clients(state) {
            tracing::error!(?e, "dispatch_clients");
        }
        let _ = display.flush_clients();
        Ok(PostAction::Continue)
    }).map_err(|e| anyhow::anyhow!("insert wayland source: {e}"))?;

    // Winit input/redraw source.
    event_loop.handle().insert_source(winit_evl, move |event, _, state| {
        match event {
            WinitEvent::Resized { size, .. } => {
                if let Some(primary) = state.primary_output.as_ref() {
                    let mode = Mode { size, refresh: 60_000 };
                    primary.output.change_current_state(Some(mode), None, None, None);
                }
            }
            WinitEvent::Input(ev) => state.process_input_event(ev),
            WinitEvent::CloseRequested => state.loop_signal.stop(),
            WinitEvent::Redraw => {
                crate::render::render_frame_winit(&mut backend, state);
            }
            _ => {}
        }
        backend.window().request_redraw();
    }).map_err(|e| anyhow::anyhow!("insert winit source: {e}"))?;

    // Seat: keyboard + pointer.
    {
        use smithay::input::keyboard::XkbConfig;
        let xkb = XkbConfig {
            layout:  &state.config.input.xkb_layout,
            variant: &state.config.input.xkb_variant,
            options: if state.config.input.xkb_options.is_empty() {
                None
            } else {
                Some(state.config.input.xkb_options.clone())
            },
            ..XkbConfig::default()
        };
        if let Err(e) = state.seat.add_keyboard(
            xkb,
            state.config.input.repeat_delay,
            state.config.input.repeat_rate,
        ) {
            tracing::error!(?e, "add_keyboard failed");
        }
        let _ = state.seat.add_pointer();
    }

    // Signals.
    crate::signals::install(&event_loop.handle())?;

    // IPC server.
    let (ipc_socket, ipc_tx) = crate::ipc_server::start(&event_loop.handle())?;
    state.ipc_request_tx = Some(ipc_tx);

    // XWayland.
    if state.config.compositor.xwayland {
        if let Err(e) = state.xwm.start(&dh) {
            tracing::error!(?e, "xwayland start failed");
        }
    }

    backend.window().request_redraw();
    info!("FRAME (winit): entering event loop");
    let result = event_loop.run(None, &mut state, |_state| {});
    info!(?state.shutdown_reason, "FRAME shutting down");
    drop(ipc_socket); // unlink control socket
    let _ = std::fs::remove_file(ipc::path::wayland_display_path());
    result.map_err(|e| anyhow::anyhow!("event loop terminated: {e}"))
}

fn publish_wayland_display(name: &str) -> Result<()> {
    let path = ipc::path::wayland_display_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Atomic rename to avoid partial reads.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, name)?;
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    std::fs::rename(&tmp, &path)?;
    std::env::set_var("WAYLAND_DISPLAY", name);
    Ok(())
}
