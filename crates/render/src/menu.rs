//! Desktop context menu shaping and rasterization via `cosmic-text` + `tiny-skia`
//! matching Waybar Everforest theme with graphical vector checkboxes and cascading submenus.

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
    pub submenu_id: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct MenuCard {
    pub entries: Vec<MenuEntry>,
    pub width: u32,
}

#[derive(Debug, Clone)]
pub struct MenuStyle {
    pub bg_rgba: [u8; 4],
    pub border_rgba: [u8; 4],
    pub header_bg_rgba: [u8; 4],
    pub text_rgba: [u8; 4],
    pub subtext_rgba: [u8; 4],
    pub separator_rgba: [u8; 4],
    pub hover_bg_rgba: [u8; 4],
    pub check_bg_rgba: [u8; 4],
    pub check_tick_rgba: [u8; 4],
    pub uncheck_bg_rgba: [u8; 4],
    pub uncheck_border_rgba: [u8; 4],
    pub disabled_rgba: [u8; 4],
    pub font_px: f32,
    pub corner_radius: f32,
    pub width: u32,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            // Waybar Everforest Theme Palette
            bg_rgba: [45, 53, 59, 252],               // #2D353B
            border_rgba: [71, 82, 88, 255],           // #475258
            header_bg_rgba: [35, 42, 46, 255],        // #232A2E
            text_rgba: [211, 198, 170, 255],          // #D3C6AA
            subtext_rgba: [133, 146, 137, 255],       // #859289
            separator_rgba: [61, 72, 77, 255],        // #3D484D
            hover_bg_rgba: [61, 72, 77, 255],         // #3D484D
            check_bg_rgba: [167, 192, 128, 255],      // #A7C080 (Everforest green)
            check_tick_rgba: [35, 42, 46, 255],       // dark tick
            uncheck_bg_rgba: [52, 63, 68, 255],       // #343F44
            uncheck_border_rgba: [92, 106, 114, 255], // #5C6A72
            disabled_rgba: [100, 112, 118, 255],
            font_px: 13.0,
            corner_radius: 8.0,
            width: 250,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderedMenu {
    pub pixmap: Pixmap,
    pub logical_bounds: (i32, i32, u32, u32), // total composite (x, y, w, h)
    pub primary_bounds: (i32, i32, u32, u32),
    pub sub_bounds: Option<(i32, i32, u32, u32)>,
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

        // Submenu items have higher priority if overlapping
        for &(id, rx, ry, rw, rh) in self.item_rects.iter().rev() {
            if cursor_x >= rx
                && cursor_x < rx + rw as i32
                && cursor_y >= ry
                && cursor_y < ry + rh as i32
            {
                return MenuHit::Item(id);
            }
        }

        // Check if inside primary or submenu card bounds
        let in_primary = cursor_x >= self.primary_bounds.0
            && cursor_x < self.primary_bounds.0 + self.primary_bounds.2 as i32
            && cursor_y >= self.primary_bounds.1
            && cursor_y < self.primary_bounds.1 + self.primary_bounds.3 as i32;

        let in_sub = self.sub_bounds.is_some_and(|sb| {
            cursor_x >= sb.0
                && cursor_x < sb.0 + sb.2 as i32
                && cursor_y >= sb.1
                && cursor_y < sb.1 + sb.3 as i32
        });

        if in_primary || in_sub {
            MenuHit::InsideNoAction
        } else {
            MenuHit::Outside
        }
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
        let mut font_system = FontSystem::new();
        for path_str in [
            "/usr/share/fonts/TTF/JetBrainsMonoNerdFontPropo-Regular.ttf",
            "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
            "/usr/share/fonts/TTF/JetBrainsMonoNerdFontPropo-Medium.ttf",
            "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Medium.ttf",
        ] {
            let p = Path::new(path_str);
            if p.is_file() {
                let _ = font_system.db_mut().load_font_file(p);
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            for dir in [
                format!("{home}/.local/share/fonts"),
                format!("{home}/.fonts"),
            ] {
                let p = Path::new(&dir);
                if p.is_dir() {
                    font_system.db_mut().load_fonts_dir(p);
                }
            }
        }

        Self {
            font_system,
            swash_cache: SwashCache::new(),
        }
    }

    pub fn load_font_file(&mut self, path: &Path) {
        let _ = self.font_system.db_mut().load_font_file(path);
    }

    /// Render primary menu card and optional cascading submenu side-by-side into a single `RenderedMenu`.
    #[must_use]
    pub fn render_menu(
        &mut self,
        primary_entries: &[MenuEntry],
        primary_pos: (i32, i32),
        submenu: Option<(&[MenuEntry], (i32, i32), u32)>, // (entries, pos, width)
        hovered_id: Option<u32>,
        style: &MenuStyle,
    ) -> Option<RenderedMenu> {
        if primary_entries.is_empty() {
            return None;
        }

        let prim_w = style.width.max(160);
        let pad_x = 10_i32;
        let pad_y = 6_i32;

        let mut item_rects = Vec::new();
        let mut cur_y = pad_y;

        for entry in primary_entries {
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
                    primary_pos.0 + pad_x,
                    primary_pos.1 + cur_y,
                    (prim_w as i32 - pad_x * 2).max(1) as u32,
                    row_h as u32,
                ));
            }
            cur_y += row_h;
        }

        let prim_h = (cur_y + pad_y).max(20) as u32;
        let prim_bounds = (primary_pos.0, primary_pos.1, prim_w, prim_h);

        // Submenu layout if active
        let sub_info = submenu.and_then(|(sub_entries, sub_pos, sub_w)| {
            if sub_entries.is_empty() {
                return None;
            }
            let mut s_y = pad_y;
            for entry in sub_entries {
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
                        sub_pos.0 + pad_x,
                        sub_pos.1 + s_y,
                        (sub_w as i32 - pad_x * 2).max(1) as u32,
                        row_h as u32,
                    ));
                }
                s_y += row_h;
            }
            let s_h = (s_y + pad_y).max(20) as u32;
            Some((sub_entries, sub_pos, sub_w, s_h))
        });

        // Compute total composite bounding box
        let min_x = if let Some((_, sp, _, _)) = sub_info {
            primary_pos.0.min(sp.0)
        } else {
            primary_pos.0
        };

        let min_y = if let Some((_, sp, _, _)) = sub_info {
            primary_pos.1.min(sp.1)
        } else {
            primary_pos.1
        };

        let max_x = if let Some((_, sp, sw, _)) = sub_info {
            (primary_pos.0 + prim_w as i32).max(sp.0 + sw as i32)
        } else {
            primary_pos.0 + prim_w as i32
        };

        let max_y = if let Some((_, sp, _, sh)) = sub_info {
            (primary_pos.1 + prim_h as i32).max(sp.1 + sh as i32)
        } else {
            primary_pos.1 + prim_h as i32
        };

        let total_w = (max_x - min_x).max(1) as u32;
        let total_h = (max_y - min_y).max(1) as u32;
        let mut pixmap = Pixmap::new(total_w, total_h)?;

        // Render primary card
        let p_rel_x = (primary_pos.0 - min_x).max(0);
        let p_rel_y = (primary_pos.1 - min_y).max(0);
        self.render_card_into(
            &mut pixmap,
            p_rel_x,
            p_rel_y,
            prim_w,
            prim_h,
            primary_entries,
            hovered_id,
            style,
        );

        // Render submenu card if active
        let sub_bounds = if let Some((sub_entries, sp, sw, sh)) = sub_info {
            let s_rel_x = (sp.0 - min_x).max(0);
            let s_rel_y = (sp.1 - min_y).max(0);
            self.render_card_into(
                &mut pixmap,
                s_rel_x,
                s_rel_y,
                sw,
                sh,
                sub_entries,
                hovered_id,
                style,
            );
            Some((sp.0, sp.1, sw, sh))
        } else {
            None
        };

        Some(RenderedMenu {
            pixmap,
            logical_bounds: (min_x, min_y, total_w, total_h),
            primary_bounds: prim_bounds,
            sub_bounds,
            item_rects,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn render_card_into(
        &mut self,
        pixmap: &mut Pixmap,
        card_x: i32,
        card_y: i32,
        card_w: u32,
        card_h: u32,
        entries: &[MenuEntry],
        hovered_id: Option<u32>,
        style: &MenuStyle,
    ) {
        let pad_x = 10_i32;
        let pad_y = 6_i32;

        // Draw card background
        if let Some(card_path) = build_rounded_rect_path(
            card_x as f32 + 0.5,
            card_y as f32 + 0.5,
            (card_x + card_w as i32) as f32 - 0.5,
            (card_y + card_h as i32) as f32 - 0.5,
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

        let mut y = card_y + pad_y;
        let font_px = style.font_px.max(11.0);
        let line_h = (font_px * 1.3).ceil();

        for entry in entries {
            if entry.is_header {
                let header_h = 30_i32;
                let mut hfill = Paint {
                    anti_alias: true,
                    ..Paint::default()
                };
                let [hr, hg, hb, ha] = style.header_bg_rgba;
                hfill.set_color(Color::from_rgba8(hr, hg, hb, ha));
                if let Some(rect) = tiny_skia::Rect::from_xywh(
                    card_x as f32 + 2.0,
                    y as f32,
                    card_w as f32 - 4.0,
                    header_h as f32,
                ) {
                    pixmap.fill_rect(rect, &hfill, Transform::identity(), None);
                }

                self.draw_text_line(
                    pixmap,
                    &entry.label,
                    card_x + pad_x + 4,
                    y + 5,
                    font_px * 1.05,
                    line_h,
                    style.text_rgba,
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
                    (card_x + pad_x) as f32,
                    mid_y as f32,
                    (card_w as i32 - pad_x * 2) as f32,
                    1.0,
                ) {
                    pixmap.fill_rect(rect, &sep_paint, Transform::identity(), None);
                }
                y += sep_h;
            } else {
                let row_h = 25_i32;
                let is_hovered = hovered_id == Some(entry.id) && !entry.disabled;

                // Draw hover pill background
                if is_hovered
                    && let Some(pill_path) = build_rounded_rect_path(
                        (card_x + pad_x - 3) as f32,
                        y as f32,
                        (card_x + card_w as i32 - pad_x + 3) as f32,
                        (y + row_h) as f32,
                        4.0,
                    )
                {
                    let mut h_paint = Paint {
                        anti_alias: true,
                        ..Paint::default()
                    };
                    let [hr, hg, hb, ha] = style.hover_bg_rgba;
                    h_paint.set_color(Color::from_rgba8(hr, hg, hb, ha));
                    pixmap.fill_path(
                        &pill_path,
                        &h_paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }

                let text_color = if entry.disabled {
                    style.disabled_rgba
                } else {
                    style.text_rgba
                };

                let mut label_x = card_x + pad_x + 4;

                // Render vector-drawn square checkbox with tick mark if toggle item
                if let Some(checked) = entry.checked {
                    draw_checkbox(pixmap, label_x as f32, y as f32 + 5.0, checked, style);
                    label_x += 24;
                }

                // Draw label
                self.draw_text_line(
                    pixmap,
                    &entry.label,
                    label_x,
                    y + 3,
                    font_px,
                    line_h,
                    text_color,
                );

                // Draw right-aligned subtext (price, level, or >)
                if let Some(sub) = &entry.subtext {
                    let sub_color = if entry.disabled {
                        style.disabled_rgba
                    } else {
                        style.subtext_rgba
                    };
                    let sub_w = (sub.len() as f32 * font_px * 0.58).ceil() as i32;
                    let sub_x = (card_x + card_w as i32 - pad_x - sub_w - 4).max(label_x + 30);
                    self.draw_text_line(
                        pixmap,
                        sub,
                        sub_x,
                        y + 3,
                        font_px * 0.92,
                        line_h,
                        sub_color,
                    );
                }

                y += row_h;
            }
        }
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
    ) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_px, line_h));
        buffer.set_size(Some(pixmap.width() as f32), None);
        buffer.set_text(
            trimmed,
            &Attrs::new()
                .family(Family::Name("JetBrainsMono Nerd Font Propo"))
                .family(Family::Name("JetBrainsMono Nerd Font"))
                .family(Family::Name("FiraCode Nerd Font"))
                .family(Family::Monospace),
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

fn draw_checkbox(pixmap: &mut Pixmap, x: f32, y: f32, checked: bool, style: &MenuStyle) {
    let size = 14.0_f32;
    let radius = 3.0_f32;
    let Some(path) = build_rounded_rect_path(x, y, x + size, y + size, radius) else {
        return;
    };

    let mut fill = Paint {
        anti_alias: true,
        ..Paint::default()
    };

    if checked {
        let [cr, cg, cb, ca] = style.check_bg_rgba;
        fill.set_color(Color::from_rgba8(cr, cg, cb, ca));
        pixmap.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);

        // Draw crisp checkmark tick: line from (x+3.2, y+7.2) to (x+5.8, y+10.2) to (x+10.8, y+3.8)
        let mut pb = PathBuilder::new();
        pb.move_to(x + 3.2, y + 7.2);
        pb.line_to(x + 5.8, y + 10.2);
        pb.line_to(x + 10.8, y + 3.8);
        if let Some(tick_path) = pb.finish() {
            let mut tick_paint = Paint {
                anti_alias: true,
                ..Paint::default()
            };
            let [tr, tg, tb, ta] = style.check_tick_rgba;
            tick_paint.set_color(Color::from_rgba8(tr, tg, tb, ta));
            let stroke = Stroke {
                width: 1.8,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            pixmap.stroke_path(
                &tick_path,
                &tick_paint,
                &stroke,
                Transform::identity(),
                None,
            );
        }
    } else {
        let [ur, ug, ub, ua] = style.uncheck_bg_rgba;
        fill.set_color(Color::from_rgba8(ur, ug, ub, ua));
        pixmap.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);

        let mut border = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        let [br, bg, bb, ba] = style.uncheck_border_rgba;
        border.set_color(Color::from_rgba8(br, bg, bb, ba));
        let stroke = Stroke {
            width: 1.2,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &border, &stroke, Transform::identity(), None);
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
                submenu_id: None,
            },
            MenuEntry {
                id: 1,
                label: "Show Pet".to_owned(),
                subtext: None,
                checked: Some(true),
                disabled: false,
                is_separator: false,
                is_header: false,
                submenu_id: None,
            },
            MenuEntry {
                id: 2,
                label: "".to_owned(),
                subtext: None,
                checked: None,
                disabled: false,
                is_separator: true,
                is_header: false,
                submenu_id: None,
            },
            MenuEntry {
                id: 3,
                label: "Quick Snack".to_owned(),
                subtext: Some("$5.00".to_owned()),
                checked: None,
                disabled: false,
                is_separator: false,
                is_header: false,
                submenu_id: None,
            },
            MenuEntry {
                id: 4,
                label: "Quit".to_owned(),
                subtext: None,
                checked: None,
                disabled: false,
                is_separator: false,
                is_header: false,
                submenu_id: None,
            },
        ];

        let menu = renderer
            .render_menu(&entries, (100, 200), None, None, &style)
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
