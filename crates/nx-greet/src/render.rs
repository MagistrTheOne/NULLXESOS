//! NX-GREET UI — username + password fields on a centred panel.

use theme::{
    color,
    text::{FontVariant, TextRenderer},
};

pub const W: u32 = 1920;
pub const H: u32 = 1080;
const DEFAULT_BG_PATH: &str = "/usr/share/nullxes/backgrounds/default.png";

#[derive(Debug, Clone)]
pub struct BackgroundImage {
    width:  u32,
    height: u32,
    pixels: Vec<u8>, // RGBA8
}

impl BackgroundImage {
    fn sample(&self, x: u32, y: u32) -> [u8; 4] {
        if self.width == 0 || self.height == 0 {
            return [0, 0, 0, 255];
        }
        let sx = x.min(self.width.saturating_sub(1));
        let sy = y.min(self.height.saturating_sub(1));
        let idx = ((sy * self.width + sx) * 4) as usize;
        if idx + 3 >= self.pixels.len() {
            return [0, 0, 0, 255];
        }
        [
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ]
    }
}

pub fn load_background_image() -> Option<BackgroundImage> {
    let path = std::env::var("NULLXES_GREET_BACKGROUND")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BG_PATH.to_string());

    let reader = image::ImageReader::open(&path).ok()?;
    let decoded = reader.decode().ok()?;
    let rgba = decoded.to_rgba8();
    Some(BackgroundImage {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedField { Username, Password }

pub fn paint(
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    text:   Option<&mut TextRenderer>,
    background: Option<&BackgroundImage>,
    username: &str,
    password_len: usize,
    prompt: Option<&str>,
    error:  Option<&str>,
    focus: FocusedField,
    pending: bool,
) {
    if let Some(bg) = background {
        paint_scaled_background(pixels, stride, buf_w, buf_h, bg);
    } else {
        wlcommon::shm::clear(pixels, color::BG);
    }

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

fn paint_scaled_background(
    pixels: &mut [u8],
    stride: usize,
    buf_w:  u32,
    buf_h:  u32,
    bg: &BackgroundImage,
) {
    if buf_w == 0 || buf_h == 0 || bg.width == 0 || bg.height == 0 {
        wlcommon::shm::clear(pixels, color::BG);
        return;
    }

    // Cover strategy (center-crop): preserve aspect ratio and fill full output.
    let out_aspect = buf_w as f32 / buf_h as f32;
    let bg_aspect = bg.width as f32 / bg.height as f32;
    let (crop_w, crop_h) = if bg_aspect > out_aspect {
        (((bg.height as f32 * out_aspect).round() as u32).max(1), bg.height)
    } else {
        (bg.width, ((bg.width as f32 / out_aspect).round() as u32).max(1))
    };
    let x_off = (bg.width.saturating_sub(crop_w)) / 2;
    let y_off = (bg.height.saturating_sub(crop_h)) / 2;

    for y in 0..buf_h {
        let src_y = y_off + ((y as u64 * crop_h as u64) / buf_h as u64) as u32;
        let row = y as usize * stride;
        for x in 0..buf_w {
            let src_x = x_off + ((x as u64 * crop_w as u64) / buf_w as u64) as u32;
            let [r, g, b, _a] = bg.sample(src_x, src_y);
            let idx = row + (x as usize * 4);
            if idx + 3 >= pixels.len() {
                continue;
            }
            pixels[idx] = b;
            pixels[idx + 1] = g;
            pixels[idx + 2] = r;
            pixels[idx + 3] = 0xff;
        }
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
