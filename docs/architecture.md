# NULLXES OS — Architecture

This document describes the runtime architecture of NULLXES OS 0.1.

## Process model

Hybrid: PANEL is rendered **inside** FRAME as a render-element overlay; all
other userland surfaces are external Wayland clients connecting through the
compositor's wayland socket.

```
Hardware (UEFI x86_64)
└── linux-lts + glibc + systemd
    ├── PipeWire + WirePlumber  (audio)
    ├── NetworkManager          (networking)
    ├── systemd-logind          (seat / session)
    ├── xdg-desktop-portal-wlr  (screencast / screenshot)
    ├── greetd                  (login layer)
    │     └── /usr/bin/nullxes-greet
    └── /usr/bin/frame  (FRAME compositor)
          ├── server: wl_compositor / xdg_shell / wlr-layer-shell /
          │           ext-session-lock-v1 / ext-idle-notify-v1 / xwayland-shell
          ├── in-process PANEL  (modules: workspaces, clock, battery, network, volume)
          ├── Control IPC v1 server  ($XDG_RUNTIME_DIR/nullxes/frame.sock)
          ├── tokio runtime: D-Bus listeners (UPower, NM, login1)
          └── X11Wm  (XWayland respawn with backoff)
                ├─ external clients ─→ /usr/bin/nullxes-launcher  (wlr-layer-shell overlay)
                                       /usr/bin/nullxes-slate    (xdg-shell + alacritty_terminal)
                                       /usr/bin/nullxes-lock     (ext-session-lock-v1 + PAM)
                                       /usr/bin/nullxes-notif    (D-Bus + wlr-layer-shell)
                                       /usr/bin/nullxes-settings (xdg-shell + Control IPC v1)
```

## Lifecycle invariants

- FRAME owns `$XDG_RUNTIME_DIR/nullxes/wayland-display` and
  `$XDG_RUNTIME_DIR/nullxes/frame.sock`. Both are 0600, atomically published,
  and removed on clean shutdown.
- Every external client uses `wlcommon::ShmDoubleBuffer` for SHM allocation
  with release tracking. Buffer pool grows up to `MAX_BUFFERS = 4` to absorb
  compositor backpressure.
- IPC v1 envelopes carry `v: 1`; servers reject unknown versions with
  `version_mismatch`.
- All async tasks have a documented owner, bounded queue, and shutdown
  signal. No `unwrap()` / `expect()` in runtime paths (CI-enforced).

## Theme & branding

`crates/theme` is the **single source of truth** for colour, spacing, motion,
and typography tokens. The `theme-export` CLI generates GTK 3/4 CSS, Qt6
palettes, Plymouth scripts, systemd-boot splash bitmaps, greetd assets, and
icon-theme entries deterministically (verified by
`crates/theme/tests/export_determinism.rs`).

`xtask check-theme` fails CI on any `Color::from_hex(0x...)` usage outside the
theme crate, preventing colour duplication across the codebase.

## Distribution

Per-component PKGBUILDs in `packaging/pkgbuilds/` produce reproducible
`*.pkg.tar.zst`s. Meta-packages `nullxes-base` / `nullxes-desktop` define the
two install profiles. The pacman repo at `repo.nullxes.os/$arch` is signed
with a key that lives only on a hardware token.

The live + installer ISO uses `archiso` + Calamares (configured in
`iso/calamares/`) with btrfs + snapper + zram swap and systemd-boot.
