# NULLXES OS — Build Stages

---

## Stage 1 — Linux-based Desktop Shell  ← CURRENT

### Crates
| Crate      | Binary              | Status |
|------------|---------------------|--------|
| `theme`    | (library)           | Done — colors, spacing, type scale, motion, text renderer |
| `frame`    | `frame`             | Done — compositor state, XDG shell, input, workspaces |
| `panel`    | `nullxes-panel`     | Done — layer-shell, SHM buffer, workspace dots, clock |
| `launcher` | `nullxes-launcher`  | Done — SHM buffer, fuzzy search, keyboard, app launch |

### Build
```bash
bash scripts/setup-dev.sh       # install deps (Debian/Arch)
bash scripts/get-fonts.sh       # download Inter + JetBrains Mono
cargo build --workspace         # debug build
bash scripts/start-nullxes.sh   # launch session
```

---

## 5-DAY SPRINT TO FIRST VISIBLE SESSION

### Day 1 — Compilation clean

**Goal:** `cargo build --workspace` exits 0 with no errors.

Tasks:
- [ ] Verify smithay 0.6 API matches render.rs (`render_output` signature, `RenderOutputResult.damage` field name)
- [ ] Verify `Space::render_elements_for_output` generic bounds match
- [ ] Fix any `WlKeyboard::KeymapEvent` variant name (may be `Keymap` in 0.31)
- [ ] Verify `zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive` variant exists
- [ ] Run `cargo clippy --workspace` and fix hard errors only
- [ ] Run `cargo test --workspace` — workspace_manager tests must pass

Likely API discrepancies to check:
```
smithay::backend::renderer::damage::OutputDamageTracker::render_output
  → may be render_output_with in some versions
smithay::desktop::space::SpaceRenderElements
  → check generic params match render_elements_for_output return type
smithay::backend::winit::WinitEvent::Redraw
  → in smithay 0.6 this may be WinitEvent::Frame
wl_keyboard::Event::KeymapEvent
  → actual variant name in wayland-client 0.31
```

### Day 2 — First visible frame

**Goal:** `NULLXES_BACKEND=winit cargo run -p frame` opens a dark window. Panel appears at the bottom.

Tasks:
- [ ] Confirm `backend.window().request_redraw()` is accessible in the winit event source closure
      (closure captures `backend` by ref — may need to restructure as `Arc<Mutex>` or extract)
- [ ] Panel: verify SHM buffer renders to a wlr-layer-shell surface visible in the winit compositor
- [ ] Panel: workspace dots appear — 9 dots with correct colours
- [ ] Panel: NX mark (4 dots) appears in launcher button area
- [ ] Verify no Wayland protocol errors in compositor stderr

Primary debugging tool:
```bash
NULLXES_LOG=frame=debug,panel=debug,smithay=debug \
    cargo run -p frame 2>&1 | tee /tmp/frame.log
# In second terminal:
WAYLAND_DISPLAY=wayland-1 cargo run -p panel
```

### Day 3 — Text renders, launcher opens

**Goal:** Clock shows in panel. Launcher opens on Super key and shows app names.

Tasks:
- [ ] Confirm `scripts/get-fonts.sh` downloads fonts successfully
- [ ] Verify `Fonts::load_from_candidates` finds them at `assets/fonts/`
- [ ] Verify fontdue baseline math: render "Gg" and check descender alignment visually
- [ ] Panel: clock shows correct UTC time (note: local time requires libc timezone)
- [ ] Frame: bind Super key in input.rs handle_keybind → spawn `nullxes-launcher`
- [ ] Launcher: renders overlay centered on screen
- [ ] Launcher: typing filters app list in real time
- [ ] Launcher: Enter launches the app

Frame spawning launcher:
```rust
// In handle_keybind, Super key case:
KeySyms::KEY_super_L | KeySyms::KEY_super_R => {
    std::process::Command::new("nullxes-launcher").spawn().ok();
    FilterResult::Intercept(())
}
```

### Day 4 — Window management works

**Goal:** Open a terminal. Move it. Switch workspaces. Close it.

Tasks:
- [ ] Verify move grab: drag window title bar repositions it correctly
- [ ] Verify Super+Q sends close to focused window
- [ ] Verify Super+1..9 switches workspace (window disappears from view)
- [ ] Server-side window decorations: title bar (32px, Surface 1 colour, 1px Accent border on focus)
- [ ] Title bar renders window title text via fontdue
- [ ] Window close button (14px × 14px circle, top-right of title bar)

Window decoration implementation: use smithay's `xdg_decoration` protocol or
draw decorations as a compositor overlay rect before window surface elements.

### Day 5 — Session reliability

**Goal:** Start session from login manager. 30 minutes stable.

Tasks:
- [ ] `scripts/install-session.sh` installs all binaries
- [ ] Login manager (ly or sddm) shows NULLXES option
- [ ] Session starts cold (no WAYLAND_DISPLAY pre-set)
- [ ] Panel IPC: FRAME notifies panel on workspace change via Unix socket
- [ ] Launcher exit (Escape) cleans up layer-shell surface properly
- [ ] FRAME handles client disconnect without panic
- [ ] FRAME handles output resize (winit window resize) without panic
- [ ] Memory: no leak over 10 workspace switches (valgrind or heaptrack)
- [ ] README with exact build + run instructions

---

## Known Technical Debt After Day 5

These are documented and deferred to Stage 2:

| Item | Deferred reason |
|------|----------------|
| Local timezone in clock | Needs libc localtime or chrono dep |
| Panel IPC live updates | Needs calloop timer source in panel event loop |
| XWayland full init | Needs DRM backend + proper Xwayland fork |
| Server-side decorations | Needs `zxdg_decoration_manager_v1` delegate |
| Fractional DPI scaling | Needs `wp_fractional_scale_v1` |
| Window overview (Super+Tab) | Needs fullscreen overlay surface |
| Snap/tiling | Needs tile layout engine |
| Multi-monitor | Needs udev output enumeration |

---

## Stage 2 — Custom Compositor / DE
- Replace smithay with custom OBSIDIAN compositor (Vulkan, C++)
- AXIOM display protocol implementation
- GPU-accelerated panel/launcher (Vulkan immediate-mode renderer)
- Full animation system using theme::motion easing curves
- Multi-monitor support

## Stage 3 — Package System / Sandbox
- FORGE package manager (Rust)
- CAST container format (STRFS sub-volume based)
- VAULT capability model
- Per-application filesystem namespace

## Stage 4 — Filesystem / Kernel Experiments
- STRATUM FS FUSE prototype (test CoW, snapshot, checksums)
- VORN scheduler class experiments as Linux kernel module
- Custom init system (NX-INIT) as PID 1 replacement

## Stage 5 — Real OS
- VORN kernel fork from Linux
- STRATUM FS in-kernel driver
- NXBoot bootloader
- Full hardware bring-up on x86-64 + AArch64
