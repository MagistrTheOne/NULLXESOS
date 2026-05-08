//! Battery indicator — reads from `ModuleSnapshot.battery_pct` populated by
//! the UPower D-Bus listener (see `system::upower`).

use theme::{color, text::{FontVariant, TextRenderer}};

use crate::panel::ModuleSnapshot;

pub fn draw(
    text:   &mut TextRenderer,
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    pen_x:  i32,
    baseline_y: i32,
    snap:   &ModuleSnapshot,
) -> i32 {
    let Some(pct) = snap.battery_pct else {
        return pen_x;
    };
    let glyph = if snap.on_ac { "⚡" } else { "·" };
    let label = format!("{glyph}{pct}%");
    let width = text.measure_text(&label, 13.0, FontVariant::Regular);
    let x = pen_x - width;
    text.draw_text(
        pixels, stride, buf_w, buf_h,
        x, baseline_y,
        &label, 13.0, color::TEXT_SECONDARY, FontVariant::Regular,
    );
    let _ = stride;
    x
}
