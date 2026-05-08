//! Draw the launcher overlay into the SHM buffer obtained from `wlcommon`.
//!
//! All drawing is done in BGRA byte order on a row-stride buffer, identical
//! to the helpers in `wlcommon::shm`. The launcher does not own the SHM
//! buffer; it only paints into the `&mut [u8]` slice handed to it.

use theme::{
    color,
    text::{FontVariant, TextRenderer},
    Theme,
};

use crate::apps::AppEntry;

pub const W: u32 = 600;
pub const H: u32 = 400;

const ROW_H:    i32 = 44;
const SEARCH_H: i32 = 52;
const PADDING:  i32 = 20;
const ICON_SZ:  i32 = 24;

pub fn paint(
    pixels:   &mut [u8],
    stride:   usize,
    buf_w:    u32,
    buf_h:    u32,
    theme:    &Theme,
    text:     Option<&mut TextRenderer>,
    query:    &str,
    results:  &[&AppEntry],
    selected: usize,
) {
    let accent = theme.accent();
    wlcommon::shm::clear(pixels, color::SURFACE_3);

    // Search row
    wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, 0, 0, W, SEARCH_H as u32, color::SURFACE_4);
    wlcommon::shm::hline(pixels, stride, buf_w, buf_h, SEARCH_H, 0, W as i32, color::DIVIDER);

    // Cursor bar (left edge of search field)
    let cursor_x = PADDING + 2;
    wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, cursor_x, PADDING, 2, (SEARCH_H - PADDING * 2) as u32, accent);

    // Result rows
    for (i, _) in results.iter().enumerate() {
        let ry = SEARCH_H + i as i32 * ROW_H;
        if i == selected {
            wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, 0, ry, W, ROW_H as u32, color::SELECTION_BG);
            wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, 0, ry, 2, ROW_H as u32, accent);
        }
        if i + 1 < results.len() {
            wlcommon::shm::hline(pixels, stride, buf_w, buf_h, ry + ROW_H - 1, PADDING, W as i32 - PADDING, color::DIVIDER);
        }
    }

    // Text — done after rectangles so text sits on top.
    let Some(text) = text else { return; };

    if query.is_empty() {
        text.draw_text(
            pixels, stride, buf_w, buf_h,
            cursor_x + 8,
            SEARCH_H / 2 + 6,
            "Type to search…",
            14.0,
            color::TEXT_DISABLED,
            FontVariant::Regular,
        );
    } else {
        text.draw_text(
            pixels, stride, buf_w, buf_h,
            cursor_x + 8,
            SEARCH_H / 2 + 6,
            query,
            14.0,
            color::TEXT_PRIMARY,
            FontVariant::Regular,
        );
    }

    for (i, app) in results.iter().enumerate() {
        let ry = SEARCH_H + i as i32 * ROW_H;
        let label_color = if i == selected {
            color::TEXT_PRIMARY
        } else {
            color::TEXT_SECONDARY
        };
        text.draw_text(
            pixels, stride, buf_w, buf_h,
            PADDING + ICON_SZ + 12,
            ry + ROW_H / 2 + 5,
            &app.name, 14.0,
            label_color,
            FontVariant::Regular,
        );
        if let Some(comment) = &app.comment {
            if !comment.is_empty() {
                let trimmed: String = comment.chars().take(60).collect();
                text.draw_text(
                    pixels, stride, buf_w, buf_h,
                    PADDING + ICON_SZ + 12,
                    ry + ROW_H / 2 + 19,
                    &trimmed, 11.0,
                    color::TEXT_DISABLED,
                    FontVariant::Regular,
                );
            }
        }
    }
}
