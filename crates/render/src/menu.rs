//! Pure-Rust desktop context menu shaping and rasterization via `cosmic-text` + `tiny-skia`.

use std::path::Path;

use cosmic_text::{
    Attrs, Buffer, Color as CtColor, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

#[derive(Debug, Clone)]
pub struct MenuEntry {
    pub id: u32,
    pub label: String,
    pub subtext: Option<String>,
    pub checked: Option<bool>,
    pub disabled: bool,
    pub is_separator: bool,
    pub is_header: bool,
}

#[derive(Debug, Clone)]
pub struct MenuStyle {
    pub bg_rgba: [u8; 4],
    pub border_rgba: [u8; 4],
    pub header_bg_rgba: [u8; 4],
    pub text_rgba: [u8; 4],
    pub subtext_rgba: [u8; 4],
    pub separator_rgba: [u8; 4],
    pub check_rgba: [u8; 4],
    pub disabled_rgba: [u8; 4],
    pub font_px: f32,
    pub corner_radius: f32,
    pub width: u32,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            bg_rgba: [30, 30, 46, 248],          // dark slate
            border_rgba: [68, 71, 90, 255],      // subtle border
            header_bg_rgba: [40, 42, 54, 255],   // header card
            text_rgba: [248, 248, 242, 255],     // white text
            subtext_rgba: [148, 163, 184, 255],  // muted text
            separator_rgba: [56, 58, 89, 255],   // divider
            check_rgba: [80, 250, 123, 255],     // checkmark
            disabled_rgba: [100, 105, 125, 255], // grayed out
            font_px: 13.0,
            corner_radius: 8.0,
            width: 250,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderedMenu {
    pub pixmap: Pixmap,
    pub logical_bounds: (i32, i32, u32, u32), // (x, y, w, h)
    pub item_rects: Vec<(u32, i32, i32, u32, u32)>, // (id, x, y, w, h)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuHit {
    Item(u32),
    InsideNoAction,
    Outside,
}

impl RenderedMenu {
    #[must_use]
    pub fn hit_test(&self, cursor_x: i32, cursor_y: i32) -> MenuHit {
        let (bx, by, bw, bh) = self.logical_bounds;
        if cursor_x < bx
            || cursor_x >= bx + bw as i32
            || cursor_y < by
            || cursor_y >= by + bh as i32
        {
            return MenuHit::Outside;
        }

        for &(id, rx, ry, rw, rh) in &self.item_rects {
            if cursor_x >= rx
                && cursor_x < rx + rw as i32
                && cursor_y >= ry
                && cursor_y < ry + rh as i32
            {
                return MenuHit::Item(id);
            }
        }

        MenuHit::InsideNoAction
    }
}

pub struct ContextMenuRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl Default for ContextMenuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextMenuRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    pub fn load_font_file(&mut self, path: &Path) {
        let _ = self.font_system.db_mut().load_font_file(path);
    }

    #[must_use]
    pub fn render_menu(
        &mut self,
        entries: &[MenuEntry],
        style: &MenuStyle,
        menu_pos: (i32, i32),
    ) -> Option<RenderedMenu> {
        if entries.is_empty() {
            return None;
        }

        let width = style.width.max(160);
        let pad_x = 10_i32;
        let pad_y = 6_i32;

        let mut item_rects = Vec::new();
        let mut cur_y = pad_y;

        for entry in entries {
            let row_h = if entry.is_header {
                30_i32
            } else if entry.is_separator {
                8_i32
            } else {
                25_i32
            };

            if !entry.is_header && !entry.is_separator && !entry.disabled {
                item_rects.push((
                    entry.id,
                    menu_pos.0 + pad_x,
                    menu_pos.1 + cur_y,
                    (width as i32 - pad_x * 2).max(1) as u32,
                    row_h as u32,
                ));
            }
            cur_y += row_h;
        }

        let total_h = (cur_y + pad_y).max(20) as u32;
        let mut pixmap = Pixmap::new(width, total_h)?;

        // 1. Draw rounded menu background card
        if let Some(card_path) = build_rounded_rect_path(
            0.5,
            0.5,
            width as f32 - 0.5,
            total_h as f32 - 0.5,
            style.corner_radius,
        ) {
            let mut fill = Paint {
                anti_alias: true,
                ..Paint::default()
            };
            let [br, bg, bb, ba] = style.bg_rgba;
            fill.set_color(Color::from_rgba8(br, bg, bb, ba));
            pixmap.fill_path(
                &card_path,
                &fill,
                FillRule::Winding,
                Transform::identity(),
                None,
            );

            let mut border = Paint {
                anti_alias: true,
                ..Paint::default()
            };
            let [sr, sg, sb, sa] = style.border_rgba;
            border.set_color(Color::from_rgba8(sr, sg, sb, sa));
            let stroke = Stroke {
                width: 1.5,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            pixmap.stroke_path(&card_path, &border, &stroke, Transform::identity(), None);
        }

        // 2. Draw each entry line
        let mut y = pad_y;
        let font_px = style.font_px.max(11.0);
        let line_h = (font_px * 1.3).ceil();

        for entry in entries {
            if entry.is_header {
                let header_h = 30_i32;
                // Draw header background strip
                let mut hfill = Paint {
                    anti_alias: true,
                    ..Paint::default()
                };
                let [hr, hg, hb, ha] = style.header_bg_rgba;
                hfill.set_color(Color::from_rgba8(hr, hg, hb, ha));
                if let Some(rect) =
                    tiny_skia::Rect::from_xywh(2.0, y as f32, width as f32 - 4.0, header_h as f32)
                {
                    pixmap.fill_rect(rect, &hfill, Transform::identity(), None);
                }

                // Render header text
                self.draw_text_line(
                    &mut pixmap,
                    &entry.label,
                    pad_x + 4,
                    y + 5,
                    font_px * 1.05,
                    line_h,
                    style.text_rgba,
                    true,
                );

                y += header_h;
            } else if entry.is_separator {
                let sep_h = 8_i32;
                let mid_y = y + sep_h / 2;
                let mut sep_paint = Paint {
                    anti_alias: false,
                    ..Paint::default()
                };
                let [sr, sg, sb, sa] = style.separator_rgba;
                sep_paint.set_color(Color::from_rgba8(sr, sg, sb, sa));
                if let Some(rect) = tiny_skia::Rect::from_xywh(
                    pad_x as f32,
                    mid_y as f32,
                    (width as i32 - pad_x * 2) as f32,
                    1.0,
                ) {
                    pixmap.fill_rect(rect, &sep_paint, Transform::identity(), None);
                }
                y += sep_h;
            } else {
                let row_h = 25_i32;
                let text_color = if entry.disabled {
                    style.disabled_rgba
                } else {
                    style.text_rgba
                };

                let mut label_x = pad_x + 4;

                // Draw checkmark or unchecked indicator if specified
                if let Some(checked) = entry.checked {
                    let indicator = if checked { "[v] " } else { "[ ] " };
                    let check_color = if checked {
                        style.check_rgba
                    } else {
                        style.disabled_rgba
                    };
                    self.draw_text_line(
                        &mut pixmap,
                        indicator,
                        label_x,
                        y + 3,
                        font_px,
                        line_h,
                        check_color,
                        false,
                    );
                    label_x += 22;
                }

                // Draw label
                self.draw_text_line(
                    &mut pixmap,
                    &entry.label,
                    label_x,
                    y + 3,
                    font_px,
                    line_h,
                    text_color,
                    false,
                );

                // Draw right-aligned subtext
                if let Some(sub) = &entry.subtext {
                    let sub_color = if entry.disabled {
                        style.disabled_rgba
                    } else {
                        style.subtext_rgba
                    };
                    let sub_w = (sub.len() as f32 * font_px * 0.58).ceil() as i32;
                    let sub_x = (width as i32 - pad_x - sub_w - 4).max(label_x + 30);
                    self.draw_text_line(
                        &mut pixmap,
                        sub,
                        sub_x,
                        y + 3,
                        font_px * 0.92,
                        line_h,
                        sub_color,
                        false,
                    );
                }

                y += row_h;
            }
        }

        Some(RenderedMenu {
            pixmap,
            logical_bounds: (menu_pos.0, menu_pos.1, width, total_h),
            item_rects,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text_line(
        &mut self,
        pixmap: &mut Pixmap,
        text: &str,
        x: i32,
        y: i32,
        font_px: f32,
        line_h: f32,
        color_rgba: [u8; 4],
        _bold: bool,
    ) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_px, line_h));
        buffer.set_size(Some(pixmap.width() as f32), None);
        buffer.set_text(
            trimmed,
            &Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let ct_color = CtColor::rgba(color_rgba[0], color_rgba[1], color_rgba[2], color_rgba[3]);
        let pw = pixmap.width() as i32;
        let ph = pixmap.height() as i32;
        let data = pixmap.data_mut();

        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            ct_color,
            |gx, gy, gw, gh, c| {
                let alpha = c.a();
                if alpha == 0 {
                    return;
                }
                for dy in 0..gh as i32 {
                    let py = y + gy + dy;
                    if py < 0 || py >= ph {
                        continue;
                    }
                    for dx in 0..gw as i32 {
                        let px = x + gx + dx;
                        if px < 0 || px >= pw {
                            continue;
                        }
                        let idx = ((py as usize) * (pw as usize) + (px as usize)) * 4;
                        blend_premultiplied_over(
                            &mut data[idx..idx + 4],
                            [c.r(), c.g(), c.b(), alpha],
                        );
                    }
                }
            },
        );
    }
}

fn build_rounded_rect_path(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    radius: f32,
) -> Option<tiny_skia::Path> {
    let r = radius.min((x1 - x0) * 0.45).min((y1 - y0) * 0.45);
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + r, y0);
    pb.line_to(x1 - r, y0);
    pb.quad_to(x1, y0, x1, y0 + r);
    pb.line_to(x1, y1 - r);
    pb.quad_to(x1, y1, x1 - r, y1);
    pb.line_to(x0 + r, y1);
    pb.quad_to(x0, y1, x0, y1 - r);
    pb.line_to(x0, y0 + r);
    pb.quad_to(x0, y0, x0 + r, y0);
    pb.close();
    pb.finish()
}

#[inline]
fn blend_premultiplied_over(dst: &mut [u8], src_straight: [u8; 4]) {
    let sa = u16::from(src_straight[3]);
    let inv_sa = 255 - sa;
    let sr = (u16::from(src_straight[0]) * sa) / 255;
    let sg = (u16::from(src_straight[1]) * sa) / 255;
    let sb = (u16::from(src_straight[2]) * sa) / 255;

    dst[0] = (sr + ((u16::from(dst[0]) * inv_sa) / 255)).min(255) as u8;
    dst[1] = (sg + ((u16::from(dst[1]) * inv_sa) / 255)).min(255) as u8;
    dst[2] = (sb + ((u16::from(dst[2]) * inv_sa) / 255)).min(255) as u8;
    dst[3] = (sa + ((u16::from(dst[3]) * inv_sa) / 255)).min(255) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_renderer_and_hit_testing() {
        let mut renderer = ContextMenuRenderer::new();
        let style = MenuStyle::default();

        let entries = vec![
            MenuEntry {
                id: 0,
                label: "Loris - Lv.51 (happy)".to_owned(),
                subtext: None,
                checked: None,
                disabled: false,
                is_separator: false,
                is_header: true,
            },
            MenuEntry {
                id: 1,
                label: "Show Pet".to_owned(),
                subtext: None,
                checked: Some(true),
                disabled: false,
                is_separator: false,
                is_header: false,
            },
            MenuEntry {
                id: 2,
                label: "".to_owned(),
                subtext: None,
                checked: None,
                disabled: false,
                is_separator: true,
                is_header: false,
            },
            MenuEntry {
                id: 3,
                label: "Quick Snack".to_owned(),
                subtext: Some("$5.00".to_owned()),
                checked: None,
                disabled: false,
                is_separator: false,
                is_header: false,
            },
            MenuEntry {
                id: 4,
                label: "Quit".to_owned(),
                subtext: None,
                checked: None,
                disabled: false,
                is_separator: false,
                is_header: false,
            },
        ];

        let menu = renderer
            .render_menu(&entries, &style, (100, 200))
            .expect("rendered menu");

        assert_eq!(menu.logical_bounds.0, 100);
        assert_eq!(menu.logical_bounds.1, 200);
        assert!(menu.logical_bounds.2 >= 200);
        assert!(menu.logical_bounds.3 >= 80);

        // Outside click
        assert_eq!(menu.hit_test(50, 50), MenuHit::Outside);
        assert_eq!(menu.hit_test(1000, 1000), MenuHit::Outside);

        // Inside header or separator
        assert_eq!(menu.hit_test(150, 210), MenuHit::InsideNoAction);

        // Click on item 1 (Show Pet)
        assert_eq!(menu.hit_test(150, 245), MenuHit::Item(1));

        // Click on item 3 (Quick Snack)
        let hit = menu.hit_test(150, 275);
        assert!(matches!(hit, MenuHit::Item(3) | MenuHit::Item(4)));
    }
}
