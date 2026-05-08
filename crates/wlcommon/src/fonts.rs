//! Font discovery for NULLXES wayland clients.
//!
//! Fonts are searched in this priority order:
//!   1. `$NULLXES_FONT_DIR` env override (for development / packaging).
//!   2. `/usr/share/fonts/nullxes/` (canonical install path, owned by `nullxes-fonts`).
//!   3. `<exe_dir>/assets/fonts/` (relative to the running binary).
//!   4. `assets/fonts/` (cwd, dev mode).
//!   5. Standard system font directories.
//!
//! Returns `None` if no Inter + JetBrains Mono pair can be located. Callers
//! should treat that as "render text-less UI" and surface a tracing warning.

use std::path::PathBuf;

use theme::text::{Fonts, TextRenderer};

pub fn load_default_text_renderer() -> Option<TextRenderer> {
    let candidates = candidate_dirs();
    for dir in &candidates {
        if let Some(fonts) = Fonts::load_from_candidates(dir) {
            tracing::info!(path = %dir.display(), "fonts loaded");
            return Some(TextRenderer::new(fonts));
        }
    }
    tracing::warn!(
        candidates = ?candidates,
        "fonts not found — install nullxes-fonts or set NULLXES_FONT_DIR",
    );
    None
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    if let Ok(env) = std::env::var("NULLXES_FONT_DIR") {
        dirs.push(PathBuf::from(env));
    }

    dirs.push(PathBuf::from("/usr/share/fonts/nullxes"));

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("assets/fonts"));
            // installed prefix layout: /usr/bin/foo → /usr/share/fonts/nullxes
            if let Some(prefix) = parent.parent() {
                dirs.push(prefix.join("share/fonts/nullxes"));
            }
        }
    }

    dirs.push(PathBuf::from("assets/fonts"));

    for d in &[
        "/usr/share/fonts/inter",
        "/usr/share/fonts/Inter",
        "/usr/share/fonts/truetype/inter",
        "/usr/share/fonts/truetype/Inter",
        "/usr/share/fonts/TTF",
        "/usr/share/fonts/jetbrains-mono",
        "/usr/share/fonts/JetBrainsMono",
        "/usr/share/fonts/truetype/jetbrains-mono",
        "/usr/share/fonts",
    ] {
        dirs.push(PathBuf::from(d));
    }

    dirs
}
