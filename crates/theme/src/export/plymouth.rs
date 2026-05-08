//! Plymouth theme export — copies the static `nullxes.plymouth` and
//! `nullxes.script` files from the packaging tree (so they are reviewable in
//! the source repo), then renders a `logo.png` from the theme tokens.

use std::path::Path;

use anyhow::Context;
use image::{ImageBuffer, Rgba, RgbaImage};

use crate::{color::*, Color, Theme};

pub fn generate(theme: &Theme, out_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir).context("create plymouth out dir")?;

    let plymouth = include_str!("../../../../packaging/plymouth/nullxes.plymouth");
    let script   = include_str!("../../../../packaging/plymouth/nullxes.script");
    std::fs::write(out_dir.join("nullxes.plymouth"), plymouth)?;
    std::fs::write(out_dir.join("nullxes.script"),   script)?;

    let logo = render_logo(theme.accent());
    logo.save(out_dir.join("logo.png")).context("write logo.png")?;
    Ok(())
}

fn render_logo(accent: Color) -> RgbaImage {
    let size: u32 = 96;
    let mut img: RgbaImage = ImageBuffer::from_pixel(size, size, color_pixel(BG));
    // 4-dot NX mark in accent colour (matches PANEL launcher button).
    let cx = size as i32 / 2;
    let cy = size as i32 / 2;
    let dot = 18i32;
    let gap = 6i32;
    let positions = [
        (cx - dot - gap / 2, cy - dot - gap / 2),
        (cx + gap / 2,       cy - dot - gap / 2),
        (cx - dot - gap / 2, cy + gap / 2),
        (cx + gap / 2,       cy + gap / 2),
    ];
    for (x, y) in positions {
        for dy in 0..dot {
            for dx in 0..dot {
                let px = x + dx;
                let py = y + dy;
                if px >= 0 && py >= 0 && (px as u32) < size && (py as u32) < size {
                    img.put_pixel(px as u32, py as u32, color_pixel(accent));
                }
            }
        }
    }
    img
}

fn color_pixel(c: Color) -> Rgba<u8> {
    Rgba([
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    ])
}
