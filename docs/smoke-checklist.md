# NULLXES OS — release-blocking smoke checklist

Must pass on every hardware target in the matrix before tagging `0.1.0`.

## Hardware matrix

| Target | Backend | Notes |
|--------|---------|-------|
| QEMU UEFI (KVM, virtio-vga, intel-hda) | DRM | primary CI target |
| Intel iGPU laptop (Tigerlake / Alderlake) | DRM | suspend/Wi-Fi/BT |
| AMD desktop (RDNA 2/3) | DRM | discrete GPU + multi-monitor regression |

NVIDIA proprietary is **not** certified for 0.1; `nullxes-nvidia-extras` is opt-in
and marked experimental.

## Functional checks

- [ ] **Cold boot ≤ 8s** to greeter (`systemd-analyze`).
- [ ] Greeter accepts password and starts FRAME session.
- [ ] PANEL renders, clock ticks once per minute, workspace dots update on `Super+1..9`.
- [ ] Two `weston-simple-egl` clients, one per workspace; switching hides the inactive.
- [ ] LAUNCHER on `Super`, accepts Cyrillic/Latin/diacritics, `Enter` launches.
- [ ] SLATE: PTY opens, shell prompts, `Ctrl+C` interrupts, `Ctrl+D` closes.
- [ ] NX-NOTIF: `notify-send "title" "body"` shows toast, auto-dismisses ~5s.
- [ ] NX-LOCK: `loginctl lock-session` → screen locks, correct password unlocks.
- [ ] Audio: `pw-play /usr/share/sounds/.../bell.oga` is audible.
- [ ] Network: NetworkManager lists Wi-Fi, connecting works.
- [ ] XWayland: `xterm` and `xeyes` start and accept input.
- [ ] Suspend: `systemctl suspend` → wake → windows intact, audio resumes.
- [ ] Browsers (Yandex / Firefox / Chrome) under XWayland render correctly.

## Performance

- [ ] No frame drops on PANEL animation at 60Hz.
- [ ] FRAME RSS < 200 MB after 30 minutes idle.
- [ ] CPU usage < 1% with no clients connected.

## Reliability

- [ ] FRAME respawns automatically if killed (`systemctl --user restart nullxes-frame`).
- [ ] XWayland respawn loop succeeds within 5s after kill.
- [ ] Lock screen enforces 120ms throttle on failed PAM attempts.
