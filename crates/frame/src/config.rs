//! FRAME runtime configuration.
//!
//! Loaded from (in priority order):
//!   1. `$NULLXES_FRAME_CONFIG` — explicit override (used by tests / CI).
//!   2. `$XDG_CONFIG_HOME/nullxes/frame.toml` (or `~/.config/...`).
//!   3. `/etc/nullxes/frame.toml` — system default installed by `nullxes-frame`.
//!
//! All fields have safe defaults; missing files are not errors.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FrameConfig {
    pub compositor: CompositorConfig,
    pub input:      InputConfig,
    pub workspace:  WorkspaceConfig,
    pub idle:       IdleConfig,
    pub keybindings: KeybindingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompositorConfig {
    pub xwayland:        bool,
    pub target_fps:      u32,
    pub vrr:             bool,
    pub lock_on_suspend: bool,
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            xwayland:        true,
            target_fps:      0, // 0 == follow display
            vrr:             true,
            lock_on_suspend: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InputConfig {
    pub natural_scroll: bool,
    pub tap_to_click:   bool,
    pub pointer_accel:  f64,
    pub repeat_delay:   i32,
    pub repeat_rate:    i32,
    pub xkb_layout:     String,
    pub xkb_variant:    String,
    pub xkb_options:    String,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            natural_scroll: true,
            tap_to_click:   true,
            pointer_accel:  0.0,
            repeat_delay:   400,
            repeat_rate:    30,
            xkb_layout:     String::new(),
            xkb_variant:    String::new(),
            xkb_options:    String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub count: u8,
    pub wrap:  bool,
}

impl Default for WorkspaceConfig {
    fn default() -> Self { Self { count: 9, wrap: false } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IdleConfig {
    /// Dim screen after this many ms of inactivity. 0 = disabled.
    pub dim_ms:     u64,
    /// Spawn `nullxes-lock` after this many ms of inactivity. 0 = disabled.
    pub lock_ms:    u64,
    /// Request system suspend after this many ms of inactivity. 0 = disabled.
    pub suspend_ms: u64,
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            dim_ms:     5  * 60 * 1000,
            lock_ms:    10 * 60 * 1000,
            suspend_ms: 30 * 60 * 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingConfig {
    pub launcher_binary: String,
    pub lock_binary:     String,
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self {
            launcher_binary: "nullxes-launcher".into(),
            lock_binary:     "nullxes-lock".into(),
        }
    }
}

impl FrameConfig {
    pub fn load() -> Self {
        let mut paths: Vec<PathBuf> = Vec::new();
        if let Ok(p) = std::env::var("NULLXES_FRAME_CONFIG") {
            paths.push(PathBuf::from(p));
        }
        if let Some(d) = dirs_next::config_dir() {
            paths.push(d.join("nullxes").join("frame.toml"));
        }
        paths.push(PathBuf::from("/etc/nullxes/frame.toml"));

        for p in &paths {
            if let Ok(text) = std::fs::read_to_string(p) {
                match toml::from_str::<FrameConfig>(&text) {
                    Ok(cfg) => {
                        tracing::info!(path = %p.display(), "config loaded");
                        return cfg;
                    }
                    Err(e) => {
                        tracing::error!(path = %p.display(), error = %e, "config parse failed; using defaults");
                    }
                }
            }
        }
        Self::default()
    }
}
