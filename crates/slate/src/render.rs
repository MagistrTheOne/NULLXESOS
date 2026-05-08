//! Render the terminal grid into a SHM buffer.
//!
//! Cell width and height are derived from JetBrains Mono at 13px. For each
//! visible cell we render the glyph + underline/strikethrough decorations on
//! top of the cell background colour. ANSI 16-colour palette is mapped to the
//! NULLXES theme so default text matches the system colour scheme; any
//! explicit palette index from alacritty_terminal still takes priority.

use alacritty_terminal::{ansi::NamedColor, grid::Indexed, term::cell::{Cell, Flags}};
use theme::{
    color,
    text::{FontVariant, TextRenderer},
};

pub const FONT_PX:    f32 = 13.0;
pub const CELL_W:     u32 = 8;   // approximate JetBrains Mono advance @ 13px
pub const CELL_H:     u32 = 18;
pub const PADDING:    u32 = 8;

pub fn paint(
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    text:   Option<&mut TextRenderer>,
    grid:   &[Vec<Cell>],
    cursor: (usize, usize),
    blink_phase: f32,
) {
    wlcommon::shm::clear(pixels, color::SURFACE_2);
    let Some(text) = text else { return; };

    let baseline_offset = (CELL_H as i32 * 3) / 4;

    for (row_idx, row) in grid.iter().enumerate() {
        let y = PADDING as i32 + (row_idx as i32) * CELL_H as i32;
        for (col_idx, cell) in row.iter().enumerate() {
            let x = PADDING as i32 + (col_idx as i32) * CELL_W as i32;
            // Cell background.
            let bg = ansi_bg(cell);
            wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, x, y, CELL_W, CELL_H, bg);

            // Glyph.
            let ch = cell.c;
            if !ch.is_control() && ch != ' ' {
                let fg = ansi_fg(cell);
                text.draw_text(
                    pixels, stride, buf_w, buf_h,
                    x, y + baseline_offset,
                    &ch.to_string(), FONT_PX,
                    fg, FontVariant::Mono,
                );
            }

            // Underline.
            if cell.flags.contains(Flags::UNDERLINE) {
                let fg = ansi_fg(cell);
                wlcommon::shm::hline(pixels, stride, buf_w, buf_h,
                    y + baseline_offset + 2, x, x + CELL_W as i32, fg);
            }
        }
    }

    // Cursor block (50% blink).
    if blink_phase > 0.5 {
        let (col, row) = cursor;
        let x = PADDING as i32 + (col as i32) * CELL_W as i32;
        let y = PADDING as i32 + (row as i32) * CELL_H as i32;
        wlcommon::shm::fill_rect(pixels, stride, buf_w, buf_h, x, y, CELL_W, CELL_H, theme::color::ACCENT.with_alpha(0.5));
    }
}

fn ansi_fg(cell: &Cell) -> theme::Color {
    use alacritty_terminal::ansi::Color as AnsiColor;
    match cell.fg {
        AnsiColor::Named(NamedColor::Foreground)        => color::TEXT_PRIMARY,
        AnsiColor::Named(NamedColor::Background)        => color::SURFACE_2,
        AnsiColor::Named(NamedColor::Black)             => color::SURFACE_1,
        AnsiColor::Named(NamedColor::Red)               => color::DESTRUCTIVE,
        AnsiColor::Named(NamedColor::Green)             => color::SUCCESS,
        AnsiColor::Named(NamedColor::Yellow)            => color::WARNING,
        AnsiColor::Named(NamedColor::Blue)              => theme::Color::from_hex(0x4B7BC9),
        AnsiColor::Named(NamedColor::Magenta)           => theme::Color::from_hex(0xA557C9),
        AnsiColor::Named(NamedColor::Cyan)              => theme::Color::from_hex(0x4CC0C9),
        AnsiColor::Named(NamedColor::White)             => color::TEXT_PRIMARY,
        AnsiColor::Spec(rgb) => theme::Color::rgb(rgb.r as f32 / 255.0, rgb.g as f32 / 255.0, rgb.b as f32 / 255.0),
        AnsiColor::Indexed(_) => color::TEXT_PRIMARY,
        _ => color::TEXT_PRIMARY,
    }
}

fn ansi_bg(cell: &Cell) -> theme::Color {
    use alacritty_terminal::ansi::Color as AnsiColor;
    match cell.bg {
        AnsiColor::Named(NamedColor::Background) => color::SURFACE_2,
        AnsiColor::Named(NamedColor::Black)      => color::SURFACE_1,
        AnsiColor::Spec(rgb) => theme::Color::rgb(rgb.r as f32 / 255.0, rgb.g as f32 / 255.0, rgb.b as f32 / 255.0),
        _ => color::SURFACE_2,
    }
}

/// Required cell-grid size for a target window pixel size.
pub fn cell_grid_for_window(window_w: u32, window_h: u32) -> (u16, u16) {
    let cols = ((window_w.saturating_sub(2 * PADDING)) / CELL_W).max(1) as u16;
    let rows = ((window_h.saturating_sub(2 * PADDING)) / CELL_H).max(1) as u16;
    (cols, rows)
}
