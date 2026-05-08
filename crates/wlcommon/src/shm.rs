//! SHM double-buffered surface for wayland-client side rendering.
//!
//! Lifecycle:
//!   ShmDoubleBuffer::new(shm, qh, w, h)
//!       → creates 2 buffers backed by 2 memfds, both marked released.
//!   draw(buffer, |pixels, stride| { … })
//!       → returns a NotReleased buffer if both are busy (caller must redraw later).
//!   On wl_buffer.release event the consumer state forwards to `mark_released`.
//!
//! Invariants:
//!   - At most one buffer is attached to a surface at a time.
//!   - The OwnedFd backing each mapping outlives the WlBuffer.
//!   - The WlShmPool covers exactly w*h*4 bytes.
//!
//! Recovery:
//!   - If a buffer is still busy at draw time, we allocate a fresh third buffer
//!     bounded by `MAX_BUFFERS`. Callers that hit the bound can either drop the
//!     redraw (most overlay UIs) or coalesce.
//!
//! GPU migration path:
//!   - Replace the inner `Buffer` with a Vulkan-backed image; the public draw()
//!     API stays. `SurfaceKind` lets us swap implementations without touching
//!     consumer crates.

use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use memmap2::MmapMut;
use wayland_client::{
    protocol::{
        wl_buffer::WlBuffer,
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    QueueHandle,
};

use crate::error::{ClientError, Result};

/// Maximum number of buffers we will allocate to absorb compositor backpressure.
/// Each buffer is `w*h*4` bytes; for a 1920×48 panel that is ~360 KiB, four of
/// those is ~1.5 MiB — well within bounds for the worst recovery case.
pub const MAX_BUFFERS: usize = 4;

/// Per-buffer release flag stored as `WlBuffer` userdata. Cloned into the
/// consumer's `Dispatch<WlBuffer, BufferUserData>` so the impl can flip it
/// when the compositor sends `Release`.
#[derive(Clone)]
pub struct BufferUserData {
    pub released: Arc<AtomicBool>,
}

impl BufferUserData {
    pub fn new(initial_released: bool) -> Self {
        Self {
            released: Arc::new(AtomicBool::new(initial_released)),
        }
    }

    pub fn mark_released(&self) {
        self.released.store(true, Ordering::Release);
    }
}

struct Buffer {
    _fd:      OwnedFd,
    pixels:   MmapMut,
    _pool:    WlShmPool,
    wl_buf:   WlBuffer,
    userdata: BufferUserData,
    w:        u32,
    h:        u32,
}

impl Buffer {
    fn is_released(&self) -> bool {
        self.userdata.released.load(Ordering::Acquire)
    }

    fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels[..]
    }
}

pub struct ShmDoubleBuffer<S: 'static + Send + Sync> {
    shm:      WlShm,
    qh:       QueueHandle<S>,
    buffers:  Vec<Buffer>,
    w:        u32,
    h:        u32,
}

impl<S: 'static + Send + Sync> ShmDoubleBuffer<S> {
    pub fn new(shm: WlShm, qh: QueueHandle<S>, w: u32, h: u32) -> Result<Self>
    where
        S: wayland_client::Dispatch<WlShmPool, ()> + wayland_client::Dispatch<WlBuffer, BufferUserData>,
    {
        let mut me = Self {
            shm,
            qh,
            buffers: Vec::with_capacity(2),
            w,
            h,
        };
        me.allocate_one()?;
        me.allocate_one()?;
        Ok(me)
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    /// Allocate one fresh buffer. Returns the index of the new buffer.
    fn allocate_one(&mut self) -> Result<usize>
    where
        S: wayland_client::Dispatch<WlShmPool, ()> + wayland_client::Dispatch<WlBuffer, BufferUserData>,
    {
        if self.buffers.len() >= MAX_BUFFERS {
            return Err(ClientError::BufferPoolExhausted { count: MAX_BUFFERS });
        }
        let stride = (self.w * 4) as i32;
        let size   = (stride as usize).saturating_mul(self.h as usize);

        let fd = create_shm_fd(size)?;
        // Safety: fd is a valid open file descriptor referring to our memfd
        // sized to exactly `size` bytes; we keep `_fd` alive for the buffer's lifetime.
        let pixels = unsafe { MmapMut::map_mut(fd.as_raw_fd()) }
            .map_err(|e| ClientError::Shm(format!("mmap: {e}")))?;

        let pool = self.shm.create_pool(
            // Safety: `fd` is a valid OwnedFd; BorrowedFd lifetime is tied to this call.
            unsafe { BorrowedFd::borrow_raw(fd.as_raw_fd()) },
            size as i32,
            &self.qh,
            (),
        );
        let userdata = BufferUserData::new(true);
        let wl_buf = pool.create_buffer(
            0, self.w as i32, self.h as i32, stride,
            Format::Argb8888,
            &self.qh,
            userdata.clone(),
        );

        self.buffers.push(Buffer {
            _fd: fd,
            pixels,
            _pool: pool,
            wl_buf,
            userdata,
            w: self.w,
            h: self.h,
        });
        Ok(self.buffers.len() - 1)
    }

    /// Resize all backing buffers to the new size. Existing in-flight buffers
    /// are dropped — caller must wait for redraw.
    pub fn resize(&mut self, w: u32, h: u32) -> Result<()>
    where
        S: wayland_client::Dispatch<WlShmPool, ()> + wayland_client::Dispatch<WlBuffer, BufferUserData>,
    {
        if w == self.w && h == self.h {
            return Ok(());
        }
        self.buffers.clear();
        self.w = w;
        self.h = h;
        self.allocate_one()?;
        self.allocate_one()?;
        Ok(())
    }

    /// Draw into a free buffer using the provided closure, then return a handle
    /// the caller can attach to a surface and commit.
    ///
    /// Returns `Err(BufferPoolExhausted)` only if all `MAX_BUFFERS` buffers are
    /// in flight simultaneously (compositor severely backed up).
    pub fn draw<F>(&mut self, paint: F) -> Result<DrawnBuffer<'_>>
    where
        F: FnOnce(&mut [u8], usize, u32, u32),
        S: wayland_client::Dispatch<WlShmPool, ()> + wayland_client::Dispatch<WlBuffer, BufferUserData>,
    {
        let idx = self.find_free_or_grow()?;
        let stride = (self.w * 4) as usize;
        let buf = &mut self.buffers[idx];
        paint(buf.pixels_mut(), stride, buf.w, buf.h);
        buf.userdata.released.store(false, Ordering::Release);
        Ok(DrawnBuffer { wl_buf: &buf.wl_buf, w: buf.w, h: buf.h })
    }

    fn find_free_or_grow(&mut self) -> Result<usize>
    where
        S: wayland_client::Dispatch<WlShmPool, ()> + wayland_client::Dispatch<WlBuffer, BufferUserData>,
    {
        for (i, b) in self.buffers.iter().enumerate() {
            if b.is_released() {
                return Ok(i);
            }
        }
        // All in flight — try to grow up to MAX_BUFFERS.
        self.allocate_one()
    }
}

pub struct DrawnBuffer<'a> {
    wl_buf: &'a WlBuffer,
    w:      u32,
    h:      u32,
}

impl<'a> DrawnBuffer<'a> {
    pub fn attach_and_commit(self, surface: &WlSurface) {
        surface.attach(Some(self.wl_buf), 0, 0);
        surface.damage_buffer(0, 0, self.w as i32, self.h as i32);
        surface.commit();
    }
}

// ── memfd creation ──────────────────────────────────────────────────────────

fn create_shm_fd(size: usize) -> Result<OwnedFd> {
    // memfd_create(2): anonymous RAM file with close-on-exec + sealing capability.
    // Safety: passing a valid C string pointer + valid flags.
    let fd = unsafe {
        libc::memfd_create(
            b"nullxes-shm\0".as_ptr() as *const libc::c_char,
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if fd < 0 {
        return Err(ClientError::Shm(format!(
            "memfd_create: {}",
            std::io::Error::last_os_error()
        )));
    }
    // Safety: fd is a valid file descriptor; setting size to non-negative value.
    let res = unsafe { libc::ftruncate(fd, size as libc::off_t) };
    if res < 0 {
        // Safety: closing a valid fd we just allocated.
        let _ = unsafe { libc::close(fd) };
        return Err(ClientError::Shm(format!(
            "ftruncate: {}",
            std::io::Error::last_os_error()
        )));
    }
    // Safety: fd is a valid open file descriptor we own; transferring ownership.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

// ── Pixel helpers (BGRA in memory for wl_shm ARGB8888 little-endian) ───────

/// Fast clear to a single colour — used as the first draw step every frame.
pub fn clear(pixels: &mut [u8], color: theme::Color) {
    let [r, g, b, a] = color.to_u8();
    let mut i = 0;
    while i + 3 < pixels.len() {
        pixels[i]     = b;
        pixels[i + 1] = g;
        pixels[i + 2] = r;
        pixels[i + 3] = a;
        i += 4;
    }
}

/// Solid (or alpha-blended) rectangle fill into a row-stride buffer.
pub fn fill_rect(
    pixels: &mut [u8], stride: usize, buf_w: u32, buf_h: u32,
    x: i32, y: i32, w: u32, h: u32,
    color: theme::Color,
) {
    let [r, g, b, a] = color.to_u8();
    let x0 = x.max(0) as usize;
    let y0 = y.max(0) as usize;
    let x1 = (x.saturating_add(w as i32)).min(buf_w as i32).max(0) as usize;
    let y1 = (y.saturating_add(h as i32)).min(buf_h as i32).max(0) as usize;
    if x0 >= x1 || y0 >= y1 { return; }

    if a == 255 {
        for py in y0..y1 {
            let row = py * stride;
            for px in x0..x1 {
                let i = row + px * 4;
                pixels[i]     = b;
                pixels[i + 1] = g;
                pixels[i + 2] = r;
                pixels[i + 3] = 255;
            }
        }
    } else {
        let src_a = a as u32;
        let dst_a = 255 - src_a;
        for py in y0..y1 {
            let row = py * stride;
            for px in x0..x1 {
                let i = row + px * 4;
                pixels[i]     = ((pixels[i]     as u32 * dst_a + b as u32 * src_a) / 255) as u8;
                pixels[i + 1] = ((pixels[i + 1] as u32 * dst_a + g as u32 * src_a) / 255) as u8;
                pixels[i + 2] = ((pixels[i + 2] as u32 * dst_a + r as u32 * src_a) / 255) as u8;
                pixels[i + 3] = 255;
            }
        }
    }
}

pub fn hline(
    pixels: &mut [u8], stride: usize, buf_w: u32, buf_h: u32,
    y: i32, x0: i32, x1: i32, color: theme::Color,
) {
    if x1 <= x0 { return; }
    fill_rect(pixels, stride, buf_w, buf_h, x0, y, (x1 - x0) as u32, 1, color);
}

pub fn vline(
    pixels: &mut [u8], stride: usize, buf_w: u32, buf_h: u32,
    x: i32, y0: i32, y1: i32, color: theme::Color,
) {
    if y1 <= y0 { return; }
    fill_rect(pixels, stride, buf_w, buf_h, x, y0, 1, (y1 - y0) as u32, color);
}
