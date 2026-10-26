//! Alpha-silhouette to Wayland `wl_region` rectangle decomposition.
//!
//! `wl_surface.set_input_region(None)` means the **entire** surface accepts input.
//! Click-through MUST pass `Some(&empty_wl_region)`. Normal mode passes `Some(&region)`
//! built from the rects returned here (strictly capped below `max_rects`, default `< 512`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectI32 {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Decompose a premultiplied RGBA buffer (`width * height * 4`) into surface-local
/// horizontal span rectangles for pixels with `alpha >= alpha_threshold`.
///
/// `scale` converts buffer-space sprite dimensions to logical surface coordinates (`>= 1`).
/// Automatically coarsens vertical band height so the returned count never exceeds `max_rects`.
pub fn alpha_mask_rects_scaled(
    rgba: &[u8],
    size: (u32, u32),
    scale: i32,
    offset: (i32, i32),
    alpha_threshold: u8,
    max_rects: usize,
) -> Vec<RectI32> {
    let (width, height) = size;
    let (offset_x, offset_y) = offset;
    if width == 0
        || height == 0
        || max_rects == 0
        || rgba.len() < (width as usize) * (height as usize) * 4
    {
        return Vec::new();
    }

    let s = scale.max(1) as u32;
    let logical_w = width.div_ceil(s);
    let logical_h = height.div_ceil(s);

    let mut band_h = 4_u32;
    loop {
        let rects = build_bands(
            rgba,
            width,
            height,
            s,
            logical_w,
            logical_h,
            band_h,
            offset_x,
            offset_y,
            alpha_threshold,
            true,
        );
        if rects.len() <= max_rects {
            return rects;
        }

        // Next try single bounding span per band before doubling band height.
        let merged = build_bands(
            rgba,
            width,
            height,
            s,
            logical_w,
            logical_h,
            band_h,
            offset_x,
            offset_y,
            alpha_threshold,
            false,
        );
        if merged.len() <= max_rects || band_h >= logical_h {
            if merged.len() <= max_rects {
                return merged;
            }
            // Final fallback: single tight bounding box around all opaque pixels.
            return bounding_rect(&merged).into_iter().collect();
        }
        band_h = band_h.saturating_mul(2);
    }
}

#[allow(clippy::too_many_arguments)]
fn build_bands(
    rgba: &[u8],
    width: u32,
    height: u32,
    scale: u32,
    logical_w: u32,
    logical_h: u32,
    band_h: u32,
    offset_x: i32,
    offset_y: i32,
    alpha_threshold: u8,
    split_runs: bool,
) -> Vec<RectI32> {
    let mut out = Vec::new();
    let stride = (width as usize) * 4;
    let mut ly = 0_u32;

    while ly < logical_h {
        let cur_band_h = band_h.min(logical_h - ly);
        let py_start = ly * scale;
        let py_end = ((ly + cur_band_h) * scale).min(height);

        if split_runs {
            let mut run_start: Option<u32> = None;
            for lx in 0..logical_w {
                let opaque = cell_is_opaque(
                    rgba,
                    stride,
                    width,
                    lx,
                    scale,
                    py_start,
                    py_end,
                    alpha_threshold,
                );
                match (run_start, opaque) {
                    (None, true) => run_start = Some(lx),
                    (Some(start), false) => {
                        out.push(RectI32 {
                            x: offset_x + start as i32,
                            y: offset_y + ly as i32,
                            w: (lx - start) as i32,
                            h: cur_band_h as i32,
                        });
                        run_start = None;
                    }
                    _ => {}
                }
            }
            if let Some(start) = run_start {
                out.push(RectI32 {
                    x: offset_x + start as i32,
                    y: offset_y + ly as i32,
                    w: (logical_w - start) as i32,
                    h: cur_band_h as i32,
                });
            }
        } else {
            let mut min_x: Option<u32> = None;
            let mut max_x = 0_u32;
            for lx in 0..logical_w {
                if cell_is_opaque(
                    rgba,
                    stride,
                    width,
                    lx,
                    scale,
                    py_start,
                    py_end,
                    alpha_threshold,
                ) {
                    min_x = Some(min_x.map_or(lx, |m| m.min(lx)));
                    max_x = lx + 1;
                }
            }
            if let Some(x0) = min_x {
                out.push(RectI32 {
                    x: offset_x + x0 as i32,
                    y: offset_y + ly as i32,
                    w: (max_x - x0) as i32,
                    h: cur_band_h as i32,
                });
            }
        }

        ly += cur_band_h;
    }

    out
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn cell_is_opaque(
    rgba: &[u8],
    stride: usize,
    width: u32,
    lx: u32,
    scale: u32,
    py_start: u32,
    py_end: u32,
    alpha_threshold: u8,
) -> bool {
    let px_start = lx * scale;
    let px_end = ((lx + 1) * scale).min(width);
    for py in py_start..py_end {
        let row_off = (py as usize) * stride;
        for px in px_start..px_end {
            if rgba[row_off + (px as usize) * 4 + 3] >= alpha_threshold {
                return true;
            }
        }
    }
    false
}

fn bounding_rect(rects: &[RectI32]) -> Option<RectI32> {
    let first = *rects.first()?;
    let mut x1 = first.x;
    let mut y1 = first.y;
    let mut x2 = first.x + first.w;
    let mut y2 = first.y + first.h;
    for r in &rects[1..] {
        x1 = x1.min(r.x);
        y1 = y1.min(r.y);
        x2 = x2.max(r.x + r.w);
        y2 = y2.max(r.y + r.h);
    }
    Some(RectI32 {
        x: x1,
        y: y1,
        w: x2 - x1,
        h: y2 - y1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpet_render::procedural_mascot_pixmap;

    #[test]
    fn transparent_image_yields_empty_rects() {
        let buf = vec![0_u8; 64 * 64 * 4];
        let rects = alpha_mask_rects_scaled(&buf, (64, 64), 1, (100, 200), 16, 511);
        assert!(rects.is_empty());
    }

    #[test]
    fn mascot_silhouette_excludes_transparent_corners_and_respects_budget() {
        let pm = procedural_mascot_pixmap(250).unwrap();
        let rects =
            alpha_mask_rects_scaled(pm.data(), (pm.width(), pm.height()), 1, (40, 60), 16, 511);
        assert!(!rects.is_empty());
        assert!(rects.len() < 512);

        // Top-left corner (40, 60) of circular/eared mascot is transparent, center (165, 200) is opaque.
        let hits = |x: i32, y: i32| {
            rects
                .iter()
                .any(|r| x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h)
        };
        assert!(!hits(40, 60));
        assert!(hits(40 + 125, 60 + 140));
    }

    #[test]
    fn checkerboard_never_exceeds_max_rects() {
        let (w, h) = (256_u32, 256_u32);
        let mut rgba = vec![0_u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                if (x + y) % 2 == 0 {
                    rgba[((y * w + x) * 4 + 3) as usize] = 255;
                }
            }
        }
        let rects = alpha_mask_rects_scaled(&rgba, (w, h), 1, (0, 0), 16, 64);
        assert!(!rects.is_empty());
        assert!(rects.len() <= 64);
    }
}
