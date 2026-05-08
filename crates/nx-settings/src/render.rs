//! Settings UI rendering.

use theme::{
    color,
    text::{FontVariant, TextRenderer},
};

use crate::state::{Section, UiState};

pub const W: u32 = 720;
pub const H: u32 = 520;
const SIDEBAR_W: i32 = 200;
const ROW_H: i32 = 40;
const PAD: i32 = 16;

pub fn paint(
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    text:   Option<&mut TextRenderer>,
    ui:     &UiState,
) {
    wlcommon::shm::clear(pixels, color::SURFACE_2);

    // Sidebar
    wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, 0, 0, SIDEBAR_W as u32, buf_h, color::SURFACE_3);
    wlcommon::shm::vline(pixels, stride, buf_w, buf_h, SIDEBAR_W, 0, buf_h as i32, color::DIVIDER);

    let Some(text) = text else { return; };
    text.draw_text(pixels, stride, buf_w, buf_h, PAD, 28,
        "NULLXES Settings", 14.0, color::TEXT_PRIMARY, FontVariant::Medium);

    let sections = [Section::Theme, Section::Input, Section::Idle, Section::About];
    for (i, sec) in sections.iter().enumerate() {
        let y = 60 + (i as i32) * ROW_H;
        let highlighted = ui.section == *sec;
        if highlighted {
            wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, 0, y, SIDEBAR_W as u32, ROW_H as u32, color::SELECTION_BG);
            wlcommon::shm::vline(pixels, stride, buf_w, buf_h, 0, y, y + ROW_H, color::ACCENT);
        }
        let label = sec.label();
        text.draw_text(
            pixels, stride, buf_w, buf_h,
            PAD,
            y + ROW_H / 2 + 5,
            label, 13.0,
            if highlighted { color::TEXT_PRIMARY } else { color::TEXT_SECONDARY },
            FontVariant::Regular,
        );
    }

    // Body
    let body_x = SIDEBAR_W + PAD;
    text.draw_text(pixels, stride, buf_w, buf_h, body_x, 32,
        ui.section.title(), 18.0, color::TEXT_PRIMARY, FontVariant::Medium);

    let mut y = 64;
    for line in ui.body_lines() {
        text.draw_text(pixels, stride, buf_w, buf_h, body_x, y,
            &line, 13.0, color::TEXT_SECONDARY, FontVariant::Regular);
        y += 22;
    }

    // Footer status
    if let Some(status) = &ui.status {
        text.draw_text(pixels, stride, buf_w, buf_h, body_x, buf_h as i32 - 16,
            status, 12.0, color::SUCCESS, FontVariant::Regular);
    }
}
