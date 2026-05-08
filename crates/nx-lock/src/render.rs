//! Render the lock surface — centred panel with avatar + password field.

use theme::{
    color,
    text::{FontVariant, TextRenderer},
};

pub fn paint(
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    text:   Option<&mut TextRenderer>,
    username: &str,
    password_len: usize,
    fail_count: u32,
    pending: bool,
) {
    wlcommon::shm::clear(pixels, color::BG);

    let panel_w = 420u32;
    let panel_h = 240u32;
    let px = ((buf_w as i32) - panel_w as i32) / 2;
    let py = ((buf_h as i32) - panel_h as i32) / 2;

    wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, px, py, panel_w, panel_h, color::SURFACE_2);
    wlcommon::shm::hline(pixels, stride, buf_w, buf_h, py, px, px + panel_w as i32, color::DIVIDER);
    wlcommon::shm::hline(pixels, stride, buf_w, buf_h, py + panel_h as i32 - 1, px, px + panel_w as i32, color::DIVIDER);

    // "Avatar" placeholder — accent dot.
    let cy = py + 56;
    let cx = px + (panel_w as i32) / 2;
    for dy in -28..=28 {
        for dx in -28..=28 {
            if dx * dx + dy * dy <= 28 * 28 {
                wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, cx + dx, cy + dy, 1, 1, color::SURFACE_4);
            }
        }
    }
    for dy in -8..=8 {
        for dx in -8..=8 {
            if dx * dx + dy * dy <= 8 * 8 {
                wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, cx + dx, cy + dy, 1, 1, color::ACCENT);
            }
        }
    }

    // Password field.
    let field_x = px + 32;
    let field_y = py + 130;
    let field_w = panel_w as i32 - 64;
    let field_h = 36i32;
    wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, field_x, field_y, field_w as u32, field_h as u32, color::SURFACE_3);
    wlcommon::shm::hline(pixels, stride, buf_w, buf_h, field_y + field_h, field_x, field_x + field_w,
        if pending { color::TEXT_DISABLED } else { color::ACCENT });

    let Some(text) = text else { return; };

    text.draw_text(
        pixels, stride, buf_w, buf_h,
        cx - text.measure_text(username, 14.0, FontVariant::Medium) / 2,
        py + 110,
        username, 14.0, color::TEXT_PRIMARY, FontVariant::Medium,
    );

    let mask: String = "•".repeat(password_len.min(32));
    text.draw_text(
        pixels, stride, buf_w, buf_h,
        field_x + 12,
        field_y + field_h / 2 + 5,
        &mask, 14.0,
        if pending { color::TEXT_DISABLED } else { color::TEXT_PRIMARY },
        FontVariant::Regular,
    );

    if fail_count > 0 {
        let label = if fail_count == 1 {
            "Incorrect password".to_string()
        } else {
            format!("{} failed attempts", fail_count)
        };
        text.draw_text(
            pixels, stride, buf_w, buf_h,
            cx - text.measure_text(&label, 12.0, FontVariant::Regular) / 2,
            py + panel_h as i32 - 24,
            &label, 12.0, color::DESTRUCTIVE, FontVariant::Regular,
        );
    }
}
