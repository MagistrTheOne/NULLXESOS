<div align="center">

<img src="assets/brand/nullxes-banner.gif" width="100%" alt="NULLXES OS Banner"/>

<h1>NULLXES VENDETA</h1>

<p>
  <b>NULLXES VENDETA</b><br/>
  Arch-based workstation OS layer with a Rust-powered Wayland userland.
</p>

<p>
  <img src="https://img.shields.io/badge/status-alpha_engineering-111111?style=for-the-badge&labelColor=000000&color=8b5cf6"/>
  <img src="https://img.shields.io/badge/rust-userland-111111?style=for-the-badge&labelColor=000000&color=f97316"/>
  <img src="https://img.shields.io/badge/wayland-native-111111?style=for-the-badge&labelColor=000000&color=38bdf8"/>
  <img src="https://img.shields.io/badge/base-arch_linux-111111?style=for-the-badge&labelColor=000000&color=1793d1"/>
</p>

</div>

---

<div align="center">

### **VENDETA EDITION**

<img src="assets/showcase/desktop-preview.gif" width="850" alt="NULLXES VENDETA Desktop Preview"/>

</div>

---

## What is NULLXES OS?

**NULLXES OS** is an experimental cyberpunk-style Linux desktop distribution built around a custom Rust userland.

It is not a generic Linux rice.  
It is not another GNOME/KDE theme.  
It is a focused desktop runtime designed for:

- custom Wayland compositor experiments
- minimal workstation workflows
- cyberpunk dark UI
- deterministic user experience
- Rust-native desktop components
- Arch-based distribution engineering

The current goal is simple:

> Build a real bootable Linux desktop distribution with NULLXES visual identity and a working Rust-powered desktop shell.

---

## System Architecture

<div align="center">

<img src="assets/architecture/nullxes-os-flow.png" width="900" alt="NULLXES OS Architecture"/>

</div>

```mermaid
flowchart TB
    hw[Hardware x86_64]
    base[Arch Base System]
    services[System Services]
    login[greetd + NULLXES Greeter]
    userland[NULLXES Rust Userland]
    dist[Pacman Repo + ArchISO]

    hw --> base
    base --> services
    services --> login
    login --> userland
    userland --> dist
    base --> dist