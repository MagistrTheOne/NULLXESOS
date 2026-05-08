//! NX-GREET UI — username + password fields on a centred panel.

use theme::{
    color,
    text::{FontVariant, TextRenderer},
};

pub const W: u32 = 1920;
pub const H: u32 = 1080;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedField { Username, Password }

pub fn paint(
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    text:   Option<&mut TextRenderer>,
    username: &str,
    password_len: usize,
    prompt: Option<&str>,
    error:  Option<&str>,
    focus: FocusedField,
    pending: bool,
) {
    wlcommon::shm::clear(pixels, color::BG);

    let panel_w = 480u32;
    let panel_h = 320u32;
    let px = ((buf_w as i32) - panel_w as i32) / 2;
    let py = ((buf_h as i32) - panel_h as i32) / 2;

    wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, px, py, panel_w, panel_h, color::SURFACE_2);

    let Some(text) = text else { return; };

    text.draw_text(pixels, stride, buf_w, buf_h, px + 24, py + 36,
        "NULLXES OS", 18.0, color::TEXT_PRIMARY, FontVariant::Medium);

    text.draw_text(pixels, stride, buf_w, buf_h, px + 24, py + 80,
        "Sign in", 14.0, color::TEXT_SECONDARY, FontVariant::Regular);

    // Username field
    draw_field(pixels, stride, buf_w, buf_h, px + 24, py + 100, panel_w as i32 - 48,
        "Username", username, focus == FocusedField::Username, pending, text, false);

    // Password field
    draw_field(pixels, stride, buf_w, buf_h, px + 24, py + 170, panel_w as i32 - 48,
        prompt.unwrap_or("Password"),
        &"•".repeat(password_len.min(64)),
        focus == FocusedField::Password, pending, text, true);

    if let Some(e) = error {
        text.draw_text(pixels, stride, buf_w, buf_h,
            px + 24, py + panel_h as i32 - 24,
            e, 12.0, color::DESTRUCTIVE, FontVariant::Regular);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_field(
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    x: i32, y: i32, w: i32,
    label: &str,
    value: &str,
    focused: bool,
    pending: bool,
    text: &mut TextRenderer,
    _password: bool,
) {
    let h = 36;
    text.draw_text(pixels, stride, buf_w, buf_h, x, y - 6,
        label, 11.0, color::TEXT_DISABLED, FontVariant::Regular);
    wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, x, y, w as u32, h as u32, color::SURFACE_3);
    let bar = if focused {
        if pending { color::TEXT_DISABLED } else { color::ACCENT }
    } else {
        color::DIVIDER
    };
    wlcommon::shm::hline(pixels, stride, buf_w, buf_h, y + h, x, x + w, bar);

    text.draw_text(pixels, stride, buf_w, buf_h,
        x + 12, y + h / 2 + 5,
        value, 14.0,
        if pending { color::TEXT_DISABLED } else { color::TEXT_PRIMARY },
        FontVariant::Regular);
}
