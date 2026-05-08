//! Qt 6 (qt6ct) palette export.

use std::path::Path;

use crate::{color::*, Color, Theme};

pub fn generate(theme: &Theme, out_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let body = palette(theme.accent());
    std::fs::write(out_dir.join("nullxes.conf"), body.into_bytes())?;
    Ok(())
}

fn rgb(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}",
        (c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8)
}

fn palette(accent: Color) -> String {
    let bg = rgb(BG);
    let surf2 = rgb(SURFACE_2);
    let surf3 = rgb(SURFACE_3);
    let surf4 = rgb(SURFACE_4);
    let txt = rgb(TEXT_PRIMARY);
    let acc = rgb(accent);
    let acc_fg = rgb(BG);
    let dis = rgb(TEXT_DISABLED);
    format!(r#"[ColorScheme]
active_colors={surf2},{txt},{surf3},{surf2},{surf4},{txt},{txt},{surf2},{txt},{bg},{bg},{txt},{acc},{acc_fg},{acc},{txt},{txt},{txt},{bg},{txt}
disabled_colors={surf2},{dis},{surf3},{surf2},{surf4},{dis},{dis},{surf2},{dis},{bg},{bg},{dis},{acc},{acc_fg},{acc},{dis},{dis},{dis},{bg},{dis}
inactive_colors={surf2},{txt},{surf3},{surf2},{surf4},{txt},{txt},{surf2},{txt},{bg},{bg},{txt},{acc},{acc_fg},{acc},{txt},{txt},{txt},{bg},{txt}
"#)
}
