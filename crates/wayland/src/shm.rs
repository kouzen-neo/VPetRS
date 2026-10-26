//! Double/triple-buffered `wl_shm` pool with per-slot dirty-rect clearing.

use smithay_client_toolkit::shm::{
    Shm,
    slot::{Buffer, SlotPool},
};
use thiserror::Error;
use vpet_render::{DamageRect, Pixmap, blit_pixmap_to_bgra, clear_bgra_rect};
use wayland_client::protocol::{wl_shm, wl_surface};

#[derive(Debug, Error)]
pub enum ShmPoolError {
    #[error("failed to create wl_shm pool: {0}")]
    CreatePool(#[from] smithay_client_toolkit::shm::CreatePoolError),
    #[error("failed to allocate shm buffer: {0}")]
    CreateBuffer(#[from] smithay_client_toolkit::shm::slot::CreateBufferError),
    #[error("failed to activate shm slot: {0}")]
    ActivateSlot(#[from] smithay_client_toolkit::shm::slot::ActivateSlotError),
    #[error("all shm buffers busy beyond swapchain cap")]
    NoAvailableBuffer,
}

struct SlotEntry {
    buffer: Buffer,
    prev_rect: Option<DamageRect>,
}

pub struct ShmSwapchain {
    pool: SlotPool,
    slots: Vec<SlotEntry>,
    buf_w: u32,
    buf_h: u32,
    last_committed_rect: Option<DamageRect>,
}

/// Result of preparing a frame in the swapchain.
pub struct PreparedFrame {
    /// Union of previous committed rect and newly drawn rect in buffer pixels.
    pub damage: DamageRect,
}

const MAX_SWAPCHAIN_SLOTS: usize = 4;

impl ShmSwapchain {
    pub fn new(shm: &Shm, buf_w: u32, buf_h: u32) -> Result<Self, ShmPoolError> {
        let initial_bytes = (buf_w.max(1) as usize) * (buf_h.max(1) as usize) * 4 * 2;
        let pool = SlotPool::new(initial_bytes, shm)?;
        Ok(Self {
            pool,
            slots: Vec::with_capacity(3),
            buf_w: buf_w.max(1),
            buf_h: buf_h.max(1),
            last_committed_rect: None,
        })
    }

    pub fn resize(&mut self, shm: &Shm, buf_w: u32, buf_h: u32) -> Result<(), ShmPoolError> {
        let w = buf_w.max(1);
        let h = buf_h.max(1);
        if self.buf_w == w && self.buf_h == h {
            return Ok(());
        }
        // Rebuild pool cleanly when output dimensions/scale change so stale buffers release safely.
        *self = Self::new(shm, w, h)?;
        Ok(())
    }

    pub const fn dimensions(&self) -> (u32, u32) {
        (self.buf_w, self.buf_h)
    }

    fn acquire_slot_index(&mut self) -> Result<usize, ShmPoolError> {
        for (idx, entry) in self.slots.iter().enumerate() {
            if entry.buffer.canvas(&mut self.pool).is_some() {
                return Ok(idx);
            }
        }
        if self.slots.len() >= MAX_SWAPCHAIN_SLOTS {
            return Err(ShmPoolError::NoAvailableBuffer);
        }
        let stride = (self.buf_w as i32) * 4;
        let (buffer, canvas) = self.pool.create_buffer(
            self.buf_w as i32,
            self.buf_h as i32,
            stride,
            wl_shm::Format::Argb8888,
        )?;
        // Ensure recycled pool regions start completely transparent.
        canvas.fill(0);
        self.slots.push(SlotEntry {
            buffer,
            prev_rect: None,
        });
        Ok(self.slots.len() - 1)
    }

    /// Draw `sprite` (or clear if `None`) into a free `wl_shm` buffer and attach it to `surface`.
    pub fn draw_and_attach(
        &mut self,
        surface: &wl_surface::WlSurface,
        sprite: Option<(&Pixmap, i32, i32)>,
    ) -> Result<PreparedFrame, ShmPoolError> {
        let idx = self.acquire_slot_index()?;
        let (buf_w, buf_h) = (self.buf_w, self.buf_h);

        let prev_in_slot = self.slots[idx].prev_rect;
        let drawn_rect = {
            let canvas = self.slots[idx]
                .buffer
                .canvas(&mut self.pool)
                .ok_or(ShmPoolError::NoAvailableBuffer)?;

            if let Some(old) = prev_in_slot {
                clear_bgra_rect(canvas, buf_w, buf_h, old);
            }

            sprite.and_then(|(pm, dst_x, dst_y)| {
                blit_pixmap_to_bgra(canvas, buf_w, buf_h, pm, dst_x, dst_y)
            })
        };

        self.slots[idx].prev_rect = drawn_rect;
        self.slots[idx].buffer.attach_to(surface)?;

        let damage = match (self.last_committed_rect, drawn_rect) {
            (Some(a), Some(b)) => a.union(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            // First empty commit after configure still needs at least 1x1 damage to map surface.
            (None, None) => DamageRect::new(0, 0, 1, 1),
        };
        self.last_committed_rect = drawn_rect;

        Ok(PreparedFrame { damage })
    }
}
