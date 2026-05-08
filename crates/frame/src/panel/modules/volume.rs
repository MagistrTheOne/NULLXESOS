//! Volume indicator — reads from `ModuleSnapshot.volume_pct/mute` populated
//! by the PipeWire listener.

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
    let Some(pct) = snap.volume_pct else { return pen_x; };
    let label = if snap.volume_mute {
        format!("MUTE")
    } else {
        format!("{pct}%")
    };
    let color = if snap.volume_mute { color::TEXT_DISABLED } else { color::TEXT_SECONDARY };
    let width = text.measure_text(&label, 13.0, FontVariant::Regular);
    let x = pen_x - width;
    text.draw_text(
        pixels, stride, buf_w, buf_h,
        x, baseline_y,
        &label, 13.0, color, FontVariant::Regular,
    );
    let _ = stride;
    x
}
