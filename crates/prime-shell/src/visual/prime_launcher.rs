use super::reference_icons;
use super::wallpaper::{paint_icon_image, DecodedWallpaper};
use crate::core_client::PreviewFileResult;
use prime_contracts::ApplicationEntry;
use std::collections::HashMap;

use super::{
    draw_icon, rail::draw_prime_mark, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem,
    Theme,
};

pub(crate) const PRIME_LAUNCHER_WIDTH: u32 = 858;
pub(crate) const PRIME_LAUNCHER_HEIGHT: u32 = 786;
pub(crate) const PRIME_LAUNCHER_TOP_MARGIN: i32 = 140;
pub(crate) const PRIME_LAUNCHER_LEFT_MARGIN: i32 = 210;
pub(crate) const fn application_state_label(launch_ready: bool) -> Option<&'static str> {
    if launch_ready {
        None
    } else {
        Some("Unavailable")
    }
}

pub(crate) const VISIBLE_APP_LIMIT: usize = 11;
pub(crate) const LAUNCHER_GLASS_ALPHA: u8 = 48;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuHit {
    App(usize),
    LeftStack,
    RightStack,
    Search,
    Outside,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PrimeLauncherLayout {
    pub(crate) bounds: Rect,
    pub(crate) search: Rect,
    pub(crate) left_stack: Rect,
    pub(crate) right_stack: Rect,
    pub(crate) center: (i32, i32),
    pub(crate) icon_size: u32,
}

impl PrimeLauncherLayout {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let cx = width as i32 / 2;
        let cy = (height as f32 * 0.50) as i32;
        let icon_size = if height < 720 { 74 } else { 76 };
        let stack_y = (height as f32 * 0.400) as i32;
        Self {
            bounds: Rect::new(0, 0, width, height),
            center: (cx, cy),
            icon_size,
            search: Rect::new(
                (width as f32 * 0.394) as i32,
                (height as f32 * 0.346) as i32,
                (width as f32 * 0.212) as u32,
                (height as f32 * 0.066).max(39.0) as u32,
            ),
            left_stack: Rect::new((width as f32 * 0.253) as i32, stack_y, 136, 87),
            right_stack: Rect::new((width as f32 * 0.675) as i32, stack_y, 136, 87),
        }
    }

    pub(crate) fn icon_rect(self, index: usize) -> Rect {
        // Pixel-measured application centers in the owner-approved 1672 x 582
        // desktop viewport. Normalized coordinates adapt to the output size.
        // They define 11 visible slots, not eleven total installed applications.
        const POINTS: [(f32, f32); VISIBLE_APP_LIMIT] = [
            (0.367, 0.480), // Music
            (0.330, 0.356), // Files
            (0.352, 0.227), // Terminal
            (0.409, 0.128), // Machine
            (0.492, 0.072), // Browser
            (0.579, 0.130), // Settings
            (0.637, 0.233), // Store
            (0.661, 0.352), // Notes
            (0.624, 0.478), // Gallery
            (0.552, 0.553), // Chat
            (0.434, 0.553), // Tools
        ];
        let (nx, ny) = POINTS[index.min(VISIBLE_APP_LIMIT - 1)];
        let cx = (self.bounds.width as f32 * nx).round() as i32;
        let cy = (self.bounds.height as f32 * ny).round() as i32;
        Rect::new(
            cx - self.icon_size as i32 / 2,
            cy - self.icon_size as i32 / 2,
            self.icon_size,
            self.icon_size,
        )
    }

    pub(crate) fn hit(self, x: f64, y: f64, count: usize, searching: bool) -> MenuHit {
        let (x, y) = (x.floor() as i32, y.floor() as i32);
        if self.search.contains(x, y) {
            return MenuHit::Search;
        }
        if !searching && count > VISIBLE_APP_LIMIT {
            if self.left_stack.contains(x, y) {
                return MenuHit::LeftStack;
            }
            if self.right_stack.contains(x, y) {
                return MenuHit::RightStack;
            }
        }
        if searching {
            // Search results are drawn as rows, not as orbit cards.
            let rows = count.min(6);
            for i in 0..rows {
                let row = Rect::new(
                    self.center.0 - 194,
                    self.search.y + self.search.height as i32 + 14 + (i as i32 * 47),
                    388,
                    43,
                );
                if row.contains(x, y) {
                    return MenuHit::App(i);
                }
            }
        } else {
            for i in 0..count.min(VISIBLE_APP_LIMIT) {
                let rect = self.icon_rect(i);
                let hitbox = Rect::new(rect.x - 10, rect.y - 10, rect.width + 20, rect.height + 38);
                if hitbox.contains(x, y) {
                    return MenuHit::App(i);
                }
            }
        }
        MenuHit::Outside
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PrimeLauncherView<'a> {
    pub(crate) applications: &'a [ApplicationEntry],
    pub(crate) icons: &'a HashMap<String, DecodedWallpaper>,
    pub(crate) incoming_left: &'a [Option<&'a DecodedWallpaper>],
    pub(crate) incoming_right: &'a [Option<&'a DecodedWallpaper>],
    pub(crate) files: &'a [PreviewFileResult],
    pub(crate) selected: usize,
    pub(crate) query: &'a str,
    pub(crate) message: Option<&'a str>,
    pub(crate) progress: f32,
    pub(crate) wallpaper_anchored: bool,
    pub(crate) overflow: bool,
}

fn application_icon(name: &str) -> Icon {
    let lower = name.to_ascii_lowercase();
    if lower.contains("machine") || lower.contains("computer") {
        Icon::Storage
    } else if lower.contains("file") || lower.contains("folder") {
        Icon::Files
    } else if lower.contains("terminal") || lower.contains("console") {
        Icon::Terminal
    } else if lower.contains("setting") || lower.contains("control") {
        Icon::Settings
    } else if lower.contains("browser")
        || lower.contains("brave")
        || lower.contains("firefox")
        || lower.contains("chrome")
    {
        Icon::Browser
    } else if lower.contains("music") || lower.contains("media") || lower.contains("video") {
        Icon::Media
    } else if lower.contains("health") || lower.contains("diagnostic") {
        Icon::Health
    } else if lower.contains("backup") || lower == "bin" {
        Icon::Recovery
    } else {
        Icon::Applications
    }
}

fn text_centered(
    text: &mut TextSystem,
    canvas: &mut Canvas<'_>,
    label: &str,
    rect: Rect,
    y: i32,
    size: u32,
    color: Argb,
) {
    let style = TextStyle {
        size_px: size as u16,
        weight: FontWeight::Semibold,
    };
    let measured = text.measure(label, style).width as i32;
    let max_chars = if rect.width < 70 { 10 } else { 18 };
    let short = if label.chars().count() > max_chars {
        let mut s = label.chars().take(max_chars - 1).collect::<String>();
        s.push('…');
        s
    } else {
        label.to_owned()
    };
    let width = text.measure(&short, style).width as i32;
    let x = rect.center_x() as i32 - width / 2;
    let _ = measured;
    text.draw(canvas, (x.max(2), y), &short, style, color);
}

fn paint_stack(
    canvas: &mut Canvas<'_>,
    theme: &Theme,
    bounds: Rect,
    right: bool,
    alpha: u8,
    artwork: &[Option<&DecodedWallpaper>],
) {
    for layer in (0..4).rev() {
        let indent = layer * 13;
        let x = if right {
            bounds.x + 12 + indent
        } else {
            bounds.x + 60 - indent
        };
        let y = bounds.y + layer * 4;
        let tile = Rect::new(x, y, 57, 61);
        canvas.fill_rounded_rect(
            tile,
            12,
            theme
                .base_2
                .with_alpha(alpha.saturating_sub(layer as u8 * 25)),
        );
        canvas.stroke_rounded_rect(
            tile,
            12,
            2,
            if right {
                theme
                    .cyan
                    .with_alpha(alpha.saturating_sub(layer as u8 * 18))
            } else {
                theme
                    .violet
                    .with_alpha(alpha.saturating_sub(layer as u8 * 18))
            },
        );
        let image = artwork.get(layer as usize).copied().flatten();
        if let Some(image) = image {
            let image_alpha = alpha.saturating_sub(layer as u8 * 20);
            paint_icon_image(canvas, image, Rect::new(x + 7, y + 7, 43, 43), image_alpha);
        } else if layer == 0 {
            draw_icon(
                canvas,
                Rect::new(x + 16, y + 15, 26, 26),
                Icon::Applications,
                theme.muted.with_alpha(alpha),
            );
        }
    }
}

pub(crate) fn paint_prime_launcher_surface(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    view: PrimeLauncherView<'_>,
) {
    canvas.clear();
    let alpha = (view.progress.clamp(0.0, 1.0) * 255.0) as u8;
    let layout = PrimeLauncherLayout::new(canvas.width, canvas.height);
    let (cx, cy) = layout.center;

    // Use the wallpaper's *existing* central logo on system-03. Other
    // wallpapers receive the exact Prime PNG/RGBA-derived mark, not a new glyph.
    if !view.wallpaper_anchored {
        draw_prime_mark(canvas, Rect::new(cx - 100, cy - 141, 200, 200), theme, true);
    }
    let ring_color = theme.cyan.with_alpha(alpha / 3);
    // Smooth, closed Catmull-Rom orbit; the former straight-edged eleven-sided
    // polygon was a visible mismatch with the owner's neon app ring.
    let positions = (0..VISIBLE_APP_LIMIT)
        .map(|i| {
            let rect = layout.icon_rect(i);
            (rect.center_x() as f32, rect.center_y() as f32)
        })
        .collect::<Vec<_>>();
    for segment in 0..positions.len() {
        let n = positions.len();
        let p0 = positions[(segment + n - 1) % n];
        let p1 = positions[segment];
        let p2 = positions[(segment + 1) % n];
        let p3 = positions[(segment + 2) % n];
        let at = |t: f32| {
            let t2 = t * t;
            let t3 = t2 * t;
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * ((2.0 * b)
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            (
                f(p0.0, p1.0, p2.0, p3.0) as i32,
                f(p0.1, p1.1, p2.1, p3.1) as i32,
            )
        };
        for step in 0..16 {
            canvas.line(
                at(step as f32 / 16.0),
                at((step + 1) as f32 / 16.0),
                1,
                ring_color,
            );
        }
    }

    let search = layout.search;
    canvas.fill_rounded_rect(
        search,
        22,
        theme.base_1.with_alpha(alpha.saturating_sub(25)),
    );
    canvas.stroke_rounded_rect(search, 22, 2, theme.cyan.with_alpha(alpha));
    draw_icon(
        canvas,
        Rect::new(search.x + 14, search.y + 12, 20, 20),
        Icon::Search,
        theme.cyan.with_alpha(alpha),
    );
    let hint = if view.query.is_empty() {
        "Search apps and files..."
    } else {
        view.query
    };
    text.draw(
        canvas,
        (search.x + 47, search.y + 11),
        hint,
        TextStyle {
            size_px: 15,
            weight: FontWeight::Regular,
        },
        theme.text.with_alpha(alpha),
    );

    if !view.query.is_empty() {
        let app_count = view.applications.len().min(6);
        let rows = (app_count + view.files.len()).min(6);
        let height = (rows * 47 + 48).max(90) as u32;
        let panel = Rect::new(cx - 205, search.y + search.height as i32 + 1, 410, height);
        canvas.fill_rounded_rect(panel, 20, theme.base_1.with_alpha(alpha.saturating_sub(7)));
        canvas.stroke_rounded_rect(panel, 20, 1, theme.cyan.with_alpha(alpha / 2));
        for i in 0..rows {
            let row = Rect::new(
                cx - 194,
                search.y + search.height as i32 + 14 + i as i32 * 47,
                388,
                43,
            );
            if i == view.selected {
                canvas.fill_rounded_rect(row, 11, theme.violet.with_alpha(alpha / 3));
            }
            let (name, icon, subtype) = if i < app_count {
                let app = &view.applications[i];
                (
                    &app.display_name,
                    application_icon(&app.display_name),
                    "App",
                )
            } else {
                let file = &view.files[i - app_count];
                (&file.name, Icon::Files, "File / indexed Ubuntu folders")
            };
            let small_rect = Rect::new(row.x + 7, row.y + 6, 29, 29);
            let artwork = if i < app_count {
                view.icons
                    .get(&view.applications[i].application_id.to_string())
            } else {
                None
            };
            let branded = if i < app_count {
                reference_icons::system_icon_for(&view.applications[i].display_name)
            } else {
                None
            };
            if let Some(image) = branded.or(artwork) {
                paint_icon_image(canvas, image, small_rect, alpha);
            } else {
                draw_icon(canvas, small_rect, icon, theme.cyan.with_alpha(alpha));
            }
            let truncated = if name.chars().count() > 31 {
                format!("{}…", name.chars().take(30).collect::<String>())
            } else {
                name.to_owned()
            };
            text.draw(
                canvas,
                (row.x + 45, row.y + 5),
                &truncated,
                TextStyle {
                    size_px: 13,
                    weight: FontWeight::Semibold,
                },
                theme.text.with_alpha(alpha),
            );
            text.draw(
                canvas,
                (row.x + 45, row.y + 25),
                subtype,
                TextStyle {
                    size_px: 10,
                    weight: FontWeight::Regular,
                },
                theme.muted.with_alpha(alpha),
            );
        }
        if rows == 0 {
            text.draw(
                canvas,
                (panel.x + 22, panel.y + 25),
                "No matching applications",
                TextStyle::body(),
                theme.muted.with_alpha(alpha),
            );
        }
        if let Some(message) = view.message {
            text.draw(
                canvas,
                (panel.x + 16, panel.y + height as i32 - 27),
                message,
                TextStyle {
                    size_px: 10,
                    weight: FontWeight::Regular,
                },
                theme.muted.with_alpha(alpha),
            );
        }
        return;
    }

    for (index, app) in view.applications.iter().take(VISIBLE_APP_LIMIT).enumerate() {
        let box_rect = layout.icon_rect(index);
        let color = if index % 2 == 0 {
            theme.cyan
        } else {
            theme.violet
        };
        if let Some(image) = reference_icons::system_icon_for(&app.display_name) {
            // The owner artwork includes the glyph and glass border. Light
            // passes around its edge; no extra generic drawn icon is added.
            canvas.radial_glow(
                box_rect.center_x() as f32,
                box_rect.center_y() as f32,
                box_rect.width as f32 * 1.12,
                color.with_alpha(alpha / 2),
            );
            reference_icons::paint_icon(canvas, image, box_rect, alpha);
        } else {
            canvas.radial_glow(
                box_rect.center_x() as f32,
                box_rect.center_y() as f32,
                box_rect.width as f32 * 0.78,
                color.with_alpha(alpha / 4),
            );
            canvas.fill_rounded_rect(
                box_rect,
                18,
                theme.base_1.with_alpha(alpha.saturating_sub(13)),
            );
            canvas.stroke_rounded_rect(box_rect, 18, 1, color.with_alpha(alpha));
            let icon_size = box_rect.width.saturating_sub(12);
            let icon_rect = Rect::new(
                box_rect.x + (box_rect.width - icon_size) as i32 / 2,
                box_rect.y + (box_rect.height - icon_size) as i32 / 2,
                icon_size,
                icon_size,
            );
            if let Some(image) = view.icons.get(&app.application_id.to_string()) {
                paint_icon_image(canvas, image, icon_rect, alpha);
            } else {
                // Explicit missing-art fallback only; never impersonate a real application's icon.
                draw_icon(
                    canvas,
                    icon_rect,
                    Icon::Applications,
                    theme.muted.with_alpha(alpha),
                );
            }
        }
        let label_box = Rect::new(
            box_rect.x - 28,
            box_rect.y,
            box_rect.width + 56,
            box_rect.height,
        );
        text_centered(
            text,
            canvas,
            &app.display_name,
            label_box,
            box_rect.y + box_rect.height as i32 + 5,
            12,
            theme.text.with_alpha(alpha),
        );
    }
    if view.overflow {
        paint_stack(
            canvas,
            theme,
            layout.left_stack,
            false,
            alpha,
            view.incoming_left,
        );
        paint_stack(
            canvas,
            theme,
            layout.right_stack,
            true,
            alpha,
            view.incoming_right,
        );
    }
    if let Some(message) = view.message {
        let panel = Rect::new(cx - 205, cy + 83, 410, 40);
        canvas.fill_rounded_rect(panel, 10, theme.base_0.with_alpha(alpha));
        text.draw(
            canvas,
            (panel.x + 10, panel.y + 12),
            message,
            TextStyle {
                size_px: 11,
                weight: FontWeight::Regular,
            },
            theme.muted.with_alpha(alpha),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approved_reference_anchor_positions_scale_to_target_output() {
        let l = PrimeLauncherLayout::new(1366, 718);
        let browser = l.icon_rect(4);
        let music = l.icon_rect(0);
        let chat = l.icon_rect(9);
        // Approved mockup's top desktop uses a 1672 px-wide coordinate
        // system. Scaling its artwork to the 1366 px output must preserve
        // that horizontal scale for BOTH axes, rather than vertically
        // stretching its 582 px viewport to the full screen.
        assert!((browser.center_x() as i32 - 672).abs() <= 2);
        assert!((browser.center_y() as i32 - 52).abs() <= 2);
        assert!((music.center_x() as i32 - 501).abs() <= 2);
        assert!((chat.center_y() as i32 - 397).abs() <= 2);
        assert!((l.search.center_x() as i32 - 684).abs() <= 4);
        assert!((l.left_stack.y - 287).abs() <= 2);
        assert!(l.icon_rect(4).y < l.icon_rect(9).y);
    }

    #[test]
    fn exactly_eleven_visible_slots_are_hit_testable_and_distinct() {
        let layout = PrimeLauncherLayout::new(1366, 768);
        let mut centers = std::collections::HashSet::new();
        for i in 0..11 {
            let r = layout.icon_rect(i);
            assert!(centers.insert((r.x, r.y)));
            assert_eq!(
                layout.hit(r.center_x(), r.center_y(), 20, false),
                MenuHit::App(i)
            );
        }
        assert_eq!(layout.hit(2.0, 740.0, 20, false), MenuHit::Outside);
    }
    #[test]
    fn two_stacks_are_not_search_or_app_buttons() {
        let l = PrimeLauncherLayout::new(1366, 768);
        assert_eq!(
            l.hit(l.left_stack.center_x(), l.left_stack.center_y(), 19, false),
            MenuHit::LeftStack
        );
        assert_eq!(
            l.hit(
                l.right_stack.center_x(),
                l.right_stack.center_y(),
                19,
                false
            ),
            MenuHit::RightStack
        );
        assert_eq!(
            l.hit(l.left_stack.center_x(), l.left_stack.center_y(), 11, false),
            MenuHit::Outside
        );
    }
}
