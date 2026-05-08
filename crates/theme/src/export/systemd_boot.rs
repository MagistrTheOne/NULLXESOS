//! systemd-boot splash bitmap — small 256×128 BMP with the NULLXES mark.
//! Outputs `splash.bmp` consumable by `loader.conf` `splash` directive.

use std::io::Cursor;
use std::path::Path;

use anyhow::Context;
use image::{ImageBuffer, ImageFormat, Rgba, RgbaImage};

use crate::{color::*, Color, Theme};

pub fn generate(theme: &Theme, out_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir).context("create systemd-boot out dir")?;
    let img = render(theme.accent());
    let mut buf: Vec<u8> = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    img.write_to(&mut cursor, ImageFormat::Bmp)?;
    std::fs::write(out_dir.join("splash.bmp"), buf)?;
    Ok(())
}

fn render(accent: Color) -> RgbaImage {
    let w: u32 = 256;
    let h: u32 = 128;
    let mut img: RgbaImage = ImageBuffer::from_pixel(w, h, color_px(BG));
    // NX mark.
    let cx = w as i32 / 2;
    let cy = h as i32 / 2;
    for (dx, dy) in [(-22i32, -22i32), (4, -22), (-22, 4), (4, 4)] {
        for y in 0..18 {
            for x in 0..18 {
                let px = cx + dx + x;
                let py = cy + dy + y;
                if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                    img.put_pixel(px as u32, py as u32, color_px(accent));
                }
            }
        }
    }
    img
}

fn color_px(c: Color) -> Rgba<u8> {
    Rgba([
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    ])
}
