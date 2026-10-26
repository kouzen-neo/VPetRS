//! Wayland layer-shell surface, input region, and shm buffer management.

pub mod fractional;
pub mod input;
pub mod output;
pub mod region;
pub mod shm;
pub mod surface;

pub use fractional::{logical_to_buffer_coord, logical_to_buffer_dim};
pub use input::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerTracker, SurfaceEvent};
pub use output::{OutputDescriptor, OutputTracker};
pub use region::{RectI32, alpha_mask_rects_scaled};
pub use shm::{PreparedFrame, ShmPoolError, ShmSwapchain};
pub use surface::{LayerMode, MAX_INPUT_REGION_RECTS, SurfaceConfig, WaylandError, WaylandShell};
