//! Network indicator — reads from `ModuleSnapshot.network` populated by the
//! NetworkManager D-Bus listener (see `system::nm`).

use theme::{color, text::{FontVariant, TextRenderer}};

use crate::panel::{ModuleSnapshot, NetworkKind};

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
    let label = match snap.network.kind {
        NetworkKind::None     => return pen_x,
        NetworkKind::Wired    => "ETH".to_string(),
        NetworkKind::Wireless => snap.network.ssid.clone().unwrap_or_else(|| "WIFI".into()),
        NetworkKind::Other    => "NET".to_string(),
    };
    if label.is_empty() { return pen_x; }
    let label = if label.len() > 16 { format!("{}…", &label[..15]) } else { label };
    let color = if snap.network.connected { color::TEXT_SECONDARY } else { color::TEXT_DISABLED };
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
