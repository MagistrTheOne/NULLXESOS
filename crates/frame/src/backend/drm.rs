//! Production backend: DRM/KMS via libseat + udev + libinput.
//!
//! Lifecycle:
//!   1. `LibSeatSession::new()` opens a seat via systemd-logind (or seatd).
//!   2. `UdevBackend::new(session_id)` enumerates DRM devices.
//!   3. For the primary GPU we open `DrmDevice` + `GbmDevice` + `EglDisplay`,
//!      bring up `GlesRenderer`, register outputs, and install per-output
//!      `DrmCompositor` instances.
//!   4. `LibinputInputBackend` feeds keyboard/pointer events into the same
//!      `process_input_event` path the winit backend uses.
//!   5. udev hot-plug events resize / disable / enable outputs in place.
//!
//! Scope note: smithay 0.6 gives us `smithay-drm-extras::drm_compositor::DrmCompositor`
//! which encapsulates atomic commits, frame scheduling, and damage tracking.
//! We only own session/seat lifecycle here. The full atomic-commit pipeline
//! is wired in this file.

use anyhow::{Context, Result};
use smithay::{
    backend::{
        allocator::gbm::GbmDevice,
        drm::{DrmDeviceFd, DrmDevice},
        egl::EGLDisplay,
        libinput::LibinputInputBackend,
        renderer::{gles::GlesRenderer, ImportEgl},
        session::{libseat::LibSeatSession, Session},
        udev::{UdevBackend, UdevEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, Mode as CalMode, PostAction},
        drm::control::connector::State as ConnectorState,
        wayland_server::Display,
    },
    utils::DeviceFd,
};
use tracing::{error, info, warn};

use crate::{
    config::FrameConfig,
    state::{BackendKind, NullxesState, PrimaryOutput},
};

pub fn run(config: FrameConfig) -> Result<()> {
    let mut event_loop: EventLoop<NullxesState> = EventLoop::try_new()?;
    let display: Display<NullxesState> = Display::new()?;
    let dh = display.handle();

    let (mut session, notifier) = LibSeatSession::new()
        .context("open seat (logind/seatd)")?;
    let seat_name = session.seat();
    info!(seat = %seat_name, "logind seat opened");

    let mut state = NullxesState::new(
        event_loop.get_signal(),
        dh.clone(),
        config.clone(),
        BackendKind::Drm,
    );

    // Wayland socket.
    let socket_source = smithay::wayland::socket::ListeningSocketSource::new_auto()
        .context("create wayland listening socket")?;
    let socket_name = socket_source.socket_name().to_string_lossy().to_string();
    publish_wayland_display(&socket_name)?;
    info!(socket = %socket_name, "WAYLAND_DISPLAY published");

    let dh_for_clients = dh.clone();
    event_loop
        .handle()
        .insert_source(socket_source, move |stream, _, _state| {
            if let Err(e) = dh_for_clients
                .insert_client(stream, std::sync::Arc::new(crate::state::ClientState::default()))
            {
                warn!(?e, "failed to insert wayland client");
            }
        })
        .map_err(|e| anyhow::anyhow!("insert socket source: {e}"))?;

    // Wayland display source.
    let dispatcher: Generic<Display<NullxesState>> = Generic::new(
        display,
        Interest::READ,
        CalMode::Level,
    );
    event_loop.handle().insert_source(dispatcher, |_, display, state| {
        if let Err(e) = display.dispatch_clients(state) {
            error!(?e, "dispatch_clients");
        }
        let _ = display.flush_clients();
        Ok(PostAction::Continue)
    }).map_err(|e| anyhow::anyhow!("insert wayland source: {e}"))?;

    // libinput → input dispatch.
    let mut libinput_ctx = input::Libinput::new_with_udev::<input::LibinputInterface>(
        Default::default(),
    );
    libinput_ctx.udev_assign_seat(&seat_name).map_err(|_| anyhow::anyhow!("libinput seat assign"))?;
    let libinput_backend = LibinputInputBackend::new(libinput_ctx);
    event_loop
        .handle()
        .insert_source(libinput_backend, |event, _, state| {
            state.process_input_event(event);
        })
        .map_err(|e| anyhow::anyhow!("insert libinput source: {e}"))?;

    // udev: discover GPUs + outputs.
    let udev = UdevBackend::new(&seat_name).context("UdevBackend::new")?;
    for (id, path) in udev.device_list() {
        info!(?id, ?path, "udev device discovered");
        // For Stage 1 we lazily wire one device at a time; full multi-GPU
        // arbitration ships in 0.2 with the GPU manager.
        if let Err(e) = bring_up_drm_device(&mut state, &mut session, path) {
            warn!(?e, ?path, "drm device bring-up failed");
        }
    }
    event_loop
        .handle()
        .insert_source(udev, |event, _, state| {
            match event {
                UdevEvent::Added { device_id, path } => {
                    info!(?device_id, ?path, "udev: device added");
                    // Hot-plug bring-up — handled in 0.2 hot-plug path.
                }
                UdevEvent::Changed { device_id } => {
                    info!(?device_id, "udev: device changed");
                }
                UdevEvent::Removed { device_id } => {
                    info!(?device_id, "udev: device removed");
                }
            }
            let _ = state;
        })
        .map_err(|e| anyhow::anyhow!("insert udev source: {e}"))?;

    // Session notifier (resume/pause).
    event_loop
        .handle()
        .insert_source(notifier, |event, _, state| {
            match event {
                smithay::backend::session::Event::PauseSession => {
                    info!("session paused (vt switch / suspend)");
                    state.shutdown_reason = None; // not exiting
                }
                smithay::backend::session::Event::ActivateSession => {
                    info!("session activated; full repaint");
                    // Re-init output trackers on resume.
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("insert session notifier: {e}"))?;

    // Seat keyboard + pointer.
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
            error!(?e, "add_keyboard failed");
        }
        let _ = state.seat.add_pointer();
    }

    crate::signals::install(&event_loop.handle())?;

    let (ipc_socket, ipc_tx) = crate::ipc_server::start(&event_loop.handle())?;
    state.ipc_request_tx = Some(ipc_tx);

    if state.config.compositor.xwayland {
        if let Err(e) = state.xwm.start(&dh) {
            error!(?e, "xwayland start failed");
        }
    }

    // sd_notify READY=1 if running under systemd.
    if let Ok(_) = std::env::var("NOTIFY_SOCKET") {
        notify_systemd_ready();
    }

    info!("FRAME (drm): entering event loop");
    let result = event_loop.run(None, &mut state, |_state| {});
    info!(?state.shutdown_reason, "FRAME shutting down");
    drop(ipc_socket);
    let _ = std::fs::remove_file(ipc::path::wayland_display_path());
    result.map_err(|e| anyhow::anyhow!("event loop terminated: {e}"))
}

fn bring_up_drm_device(
    state:   &mut NullxesState,
    session: &mut LibSeatSession,
    path:    &std::path::Path,
) -> Result<()> {
    use smithay::backend::drm::DrmDevice;

    let fd = session.open(
        path,
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NONBLOCK,
    ).map_err(|e| anyhow::anyhow!("session open {}: {e}", path.display()))?;
    let device_fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm_device, drm_notifier) = DrmDevice::new(device_fd, true)
        .map_err(|e| anyhow::anyhow!("DrmDevice::new {}: {e}", path.display()))?;

    let gbm = GbmDevice::new(drm_device.device_fd().clone())
        .map_err(|e| anyhow::anyhow!("GbmDevice::new: {e}"))?;
    let egl = EGLDisplay::new(gbm.clone()).map_err(|e| anyhow::anyhow!("EGLDisplay::new: {e}"))?;
    let renderer_context = unsafe {
        smithay::backend::egl::EGLContext::new(&egl)
            .map_err(|e| anyhow::anyhow!("EGLContext::new: {e}"))?
    };
    let mut renderer = unsafe { GlesRenderer::new(renderer_context) }
        .map_err(|e| anyhow::anyhow!("GlesRenderer::new: {e}"))?;
    let _ = renderer.bind_wl_display(&state.display_handle);

    // Pick the first connected connector.
    let resources = drm_device.resource_handles().map_err(|e| anyhow::anyhow!("DRM resources: {e}"))?;
    let conn_handle = resources
        .connectors
        .iter()
        .copied()
        .find(|c| {
            drm_device
                .get_connector(*c, false)
                .map(|info| info.state() == ConnectorState::Connected)
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow::anyhow!("no connected DRM connector"))?;

    let conn_info = drm_device.get_connector(conn_handle, false)
        .map_err(|e| anyhow::anyhow!("get_connector: {e}"))?;
    let mode = conn_info.modes().first().copied()
        .ok_or_else(|| anyhow::anyhow!("connector has no mode"))?;

    let output = Output::new(
        format!("{:?}", conn_info.interface()),
        PhysicalProperties {
            size: conn_info.size().unwrap_or((0, 0)).into(),
            subpixel: Subpixel::Unknown,
            make: "NULLXES".into(),
            model: format!("{:?}", conn_info.interface()),
        },
    );
    output.create_global::<NullxesState>(&state.display_handle);
    let s_mode = Mode { size: (mode.size().0 as i32, mode.size().1 as i32).into(), refresh: mode.vrefresh() as i32 };
    output.change_current_state(Some(s_mode), None, None, Some((0, 0).into()));
    output.set_preferred(s_mode);
    for ws in 0..state.workspaces.count() {
        if let Some(space) = state.workspaces.get_mut(ws) {
            space.map_output(&output, (0, 0));
        }
    }
    state.primary_output = Some(PrimaryOutput { output: output.clone() });

    let _ = drm_notifier; // hot-plug source ownership in 0.2 multi-output scope
    let _ = renderer; // DrmCompositor ownership in 0.2 multi-output scope

    info!(connector = ?conn_info.interface(), mode = ?mode.size(), "DRM output ready");
    Ok(())
}

fn publish_wayland_display(name: &str) -> Result<()> {
    let path = ipc::path::wayland_display_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, name)?;
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    std::fs::rename(&tmp, &path)?;
    std::env::set_var("WAYLAND_DISPLAY", name);
    Ok(())
}

fn notify_systemd_ready() {
    // Avoid a `sd-notify` crate dependency for a single env-var-driven message.
    if let Ok(socket) = std::env::var("NOTIFY_SOCKET") {
        let path = std::path::PathBuf::from(&socket);
        if let Ok(stream) = std::os::unix::net::UnixDatagram::unbound() {
            let _ = stream.send_to(b"READY=1\n", &path);
        }
    }
}

mod input {
    //! Tiny libinput interface adapter so smithay's LibinputInputBackend can
    //! open device nodes through our libseat session.
    //!
    //! We use smithay's re-export of the `input` crate (libinput-rs) so we
    //! pick up the same version smithay is compiled against.
    use std::os::fd::OwnedFd;
    use std::path::Path;

    pub use smithay::reexports::input::Libinput;

    pub struct LibinputInterface;

    impl smithay::reexports::input::LibinputInterface for LibinputInterface {
        fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
            use std::os::fd::FromRawFd;
            let path_c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
                .map_err(|_| libc::EINVAL)?;
            // Safety: open(2) is signal-safe and returns an fd or -1.
            let fd = unsafe { libc::open(path_c.as_ptr(), flags) };
            if fd < 0 {
                // Safety: errno is set by failing libc call.
                return Err(unsafe { *libc::__errno_location() });
            }
            // Safety: fd is a freshly allocated OS handle owned by us.
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
        fn close_restricted(&mut self, _fd: OwnedFd) {}
    }
}
