//! Clock module — renders local time via `nix::time::clock_gettime` so we
//! avoid pulling `chrono` into FRAME just for HH:MM. Time zone resolution
//! delegates to libc `localtime_r` which honours `/etc/localtime`.

use std::ffi::CString;

use theme::{color, text::{FontVariant, TextRenderer}};

/// Returns the new pen-x position (left edge of the rendered text).
pub fn draw(
    text:   &mut TextRenderer,
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    pen_x:  i32,
    baseline_y: i32,
) -> i32 {
    let now = format_local_hhmm();
    let width = text.measure_text(&now, 13.0, FontVariant::Regular);
    let x = pen_x - width;
    text.draw_text(
        pixels, stride, buf_w, buf_h,
        x, baseline_y,
        &now, 13.0, color::TEXT_SECONDARY, FontVariant::Regular,
    );
    let _ = stride;
    x
}

fn format_local_hhmm() -> String {
    use std::mem::MaybeUninit;

    // Safety: `time(nullptr)` returns current time, never fails on a sane system.
    let t = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: MaybeUninit<libc::tm> = MaybeUninit::uninit();

    // Safety: localtime_r writes into our zeroed `tm`. On failure returns null.
    let res = unsafe { libc::localtime_r(&t, tm.as_mut_ptr()) };
    if res.is_null() {
        // Fall back to UTC HH:MM.
        let secs = t as i64;
        let hh = (secs / 3600).rem_euclid(24);
        let mm = (secs / 60).rem_euclid(60);
        return format!("{hh:02}:{mm:02}");
    }
    // Safety: localtime_r succeeded, so `tm` is fully initialised.
    let tm = unsafe { tm.assume_init() };
    let hh = tm.tm_hour;
    let mm = tm.tm_min;
    format!("{hh:02}:{mm:02}")
}

#[allow(dead_code)]
fn _no_warn_unused() {
    // CString import retained for tzset path used by future locale code.
    let _ = CString::new("");
}
