//! Workspace indicator strip — N dots starting at x=60.
//!
//! Active dot is filled with accent. Occupied (has windows but not active)
//! dots are rendered as outlined circles in TEXT_SECONDARY. Empty dots are
//! tiny outlines in TEXT_DISABLED.

use theme::color;

use crate::panel::fill_rect;

const DOT_ACTIVE: i32 = 8;
const DOT_IDLE:   i32 = 6;
const DOT_GAP:    i32 = 10;
const START_X:    i32 = 60;

pub fn draw(
    pixels: &mut [u8], stride: usize, buf_w: u32, buf_h: u32,
    active: usize, occupied: &[usize],
) {
    let cy = buf_h as i32 / 2;
    for i in 0..9 {
        let cx = START_X + i as i32 * (DOT_IDLE + DOT_GAP) + DOT_IDLE / 2;
        if i == active {
            fill_circle(pixels, stride, buf_w, buf_h, cx, cy, DOT_ACTIVE / 2, theme::color::ACCENT);
        } else if occupied.contains(&i) {
            ring_circle(pixels, stride, buf_w, buf_h, cx, cy, DOT_IDLE / 2, color::TEXT_SECONDARY);
        } else {
            ring_circle(pixels, stride, buf_w, buf_h, cx, cy, DOT_IDLE / 2 - 1, color::TEXT_DISABLED);
        }
    }
}

fn fill_circle(pixels: &mut [u8], stride: usize, buf_w: u32, buf_h: u32, cx: i32, cy: i32, r: i32, c: theme::Color) {
    let r2 = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r2 {
                fill_rect(pixels, stride, buf_w, buf_h, cx + dx, cy + dy, 1, 1, c);
            }
        }
    }
}

fn ring_circle(pixels: &mut [u8], stride: usize, buf_w: u32, buf_h: u32, cx: i32, cy: i32, r: i32, c: theme::Color) {
    let ro2 = r * r;
    let ri2 = (r - 1) * (r - 1);
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = dx * dx + dy * dy;
            if d2 <= ro2 && d2 > ri2 {
                fill_rect(pixels, stride, buf_w, buf_h, cx + dx, cy + dy, 1, 1, c);
            }
        }
    }
}
