//! Read/write NULLXES configuration files atomically.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FrameConfFile {
    pub compositor: Compositor,
    pub input:      Input,
    pub workspace:  Workspace,
    pub idle:       Idle,
    pub keybindings: Keybindings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Compositor {
    pub xwayland: bool,
    pub target_fps: u32,
    pub vrr: bool,
    pub lock_on_suspend: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Input {
    pub natural_scroll: bool,
    pub tap_to_click: bool,
    pub pointer_accel: f64,
    pub repeat_delay: i32,
    pub repeat_rate: i32,
    pub xkb_layout: String,
    pub xkb_variant: String,
    pub xkb_options: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Workspace {
    pub count: u8,
    pub wrap: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Idle {
    pub dim_ms: u64,
    pub lock_ms: u64,
    pub suspend_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Keybindings {
    pub launcher_binary: String,
    pub lock_binary: String,
}

pub fn frame_path() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nullxes")
        .join("frame.toml")
}

pub fn theme_path() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nullxes")
        .join("theme.toml")
}

pub fn load_frame() -> FrameConfFile {
    let path = frame_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_default(),
        Err(_)   => FrameConfFile::default(),
    }
}

pub fn save_frame(cfg: &FrameConfFile) -> std::io::Result<()> {
    let path = frame_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    write_atomic(&path, s.as_bytes())
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    std::fs::rename(&tmp, path)?;
    Ok(())
}
