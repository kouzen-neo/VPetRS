//! 2D CPU sprite compositor and damage tracker.

pub mod canvas;
pub mod damage;

pub use canvas::{
    RenderError, blit_pixmap_to_bgra, clear_bgra_rect, load_sprite_scaled,
    procedural_mascot_pixmap, rgba_image_to_pixmap, scale_pixmap,
};
pub use damage::DamageRect;
pub use tiny_skia::Pixmap;
