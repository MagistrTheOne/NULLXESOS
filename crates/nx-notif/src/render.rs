//! Render a single notification toast.

use theme::{
    color,
    text::{FontVariant, TextRenderer},
};

use crate::surface::Notif;

pub const TOAST_W: u32 = 360;
pub const TOAST_H: u32 = 88;
pub const PAD:     i32 = 14;

pub fn paint(
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    text:   Option<&mut TextRenderer>,
    notif:  &Notif,
) {
    wlcommon::shm::clear(pixels, color::SURFACE_3);
    wlcommon::shm::vline(pixels, stride, buf_w, buf_h, 0, 0, buf_h as i32, accent_for(notif));

    let Some(text) = text else { return; };
    text.draw_text(
        pixels, stride, buf_w, buf_h,
        PAD + 4,
        24,
        &notif.summary, 14.0,
        color::TEXT_PRIMARY, FontVariant::Medium,
    );
    let body_first_line: String = notif.body.lines().next().unwrap_or("").chars().take(70).collect();
    if !body_first_line.is_empty() {
        text.draw_text(
            pixels, stride, buf_w, buf_h,
            PAD + 4,
            48,
            &body_first_line, 12.0,
            color::TEXT_SECONDARY, FontVariant::Regular,
        );
    }
    if !notif.app_name.is_empty() {
        text.draw_text(
            pixels, stride, buf_w, buf_h,
            PAD + 4,
            72,
            &notif.app_name, 11.0,
            color::TEXT_DISABLED, FontVariant::Regular,
        );
    }
}

fn accent_for(notif: &Notif) -> theme::Color {
    use crate::surface::Urgency;
    match notif.urgency {
        Urgency::Low      => color::TEXT_DISABLED,
        Urgency::Normal   => color::ACCENT,
        Urgency::Critical => color::DESTRUCTIVE,
    }
}
