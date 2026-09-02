pub(crate) mod primitives;
pub(crate) mod theme;

pub(crate) use primitives::{Argb, Canvas, Rect};
pub(crate) use theme::Theme;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RenderContext {
    pub(crate) theme: Theme,
}

use prime_contracts::{ApplicationEntry, SystemPowerAction};

pub(crate) const ORB_LIST_TOP: f64 = 82.0;
pub(crate) const ORB_ROW_HEIGHT: f64 = 30.0;
const QUICK_ACTION_HEIGHT: u32 = 30;
const QUICK_REBOOT_OFFSET: u32 = 112;
const QUICK_POWEROFF_OFFSET: u32 = 76;

const PANEL: u32 = 0xee171c25;
const PANEL_ALT: u32 = 0xee222936;
const SELECTED: u32 = 0xee344052;
const BORDER: u32 = 0xff64748b;
const TEXT: u32 = 0xfff3f5f7;
const MUTED: u32 = 0xffaeb8c4;
const READY: u32 = 0xffd7f2df;
const BLOCKED: u32 = 0xffffd2d2;
const ACCENT: u32 = 0xffc9d7ff;

pub(crate) fn paint_background(canvas: &mut [u8], width: u32, height: u32, base: u32) {
    for y in 0..height {
        let lift = ((y as u64 * 14) / u64::from(height.max(1))) as u8;
        let color = lift_rgb(base, lift);
        fill_rect(canvas, width, height, 0, y, width, 1, color);
    }
    let title = "PRIME";
    let scale = 4;
    let text_width = text_width(title, scale);
    let x = width.saturating_sub(text_width) / 2;
    let y = height.saturating_sub(7 * scale) / 2;
    draw_text(canvas, width, height, x, y, scale, 0x44ffffff, title);
}

pub(crate) fn paint_rail(canvas: &mut [u8], width: u32, height: u32, base: u32) {
    fill_rect(canvas, width, height, 0, 0, width, height, base);
    fill_rect(
        canvas,
        width,
        height,
        0,
        height.saturating_sub(1),
        width,
        1,
        BORDER,
    );
    draw_text(canvas, width, height, 16, 17, 2, TEXT, "PRIME");
    draw_text(canvas, width, height, 112, 19, 1, MUTED, "O  ORB");
    let right = text_width("Q  STATUS", 1).saturating_add(18);
    draw_text(
        canvas,
        width,
        height,
        width.saturating_sub(right),
        19,
        1,
        MUTED,
        "Q  STATUS",
    );
}

pub(crate) fn paint_orb(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    applications: &[ApplicationEntry],
    selected: usize,
    message: Option<&str>,
) {
    fill_rect(canvas, width, height, 0, 0, width, height, PANEL);
    stroke_rect(canvas, width, height, 0, 0, width, height, BORDER);
    draw_text(canvas, width, height, 18, 18, 2, TEXT, "PRIME ORB");
    draw_text(
        canvas,
        width,
        height,
        20,
        52,
        1,
        MUTED,
        "ADMITTED APPLICATIONS",
    );

    let max_rows = ((height.saturating_sub(150)) as f64 / ORB_ROW_HEIGHT) as usize;
    for (index, application) in applications.iter().take(max_rows).enumerate() {
        let y = ORB_LIST_TOP as u32 + (index as u32 * ORB_ROW_HEIGHT as u32);
        if index == selected {
            fill_rect(
                canvas,
                width,
                height,
                12,
                y.saturating_sub(5),
                width.saturating_sub(24),
                ORB_ROW_HEIGHT as u32 - 2,
                SELECTED,
            );
        }
        let prefix = if index == selected { ">" } else { " " };
        let state = if application.launch_ready {
            "READY"
        } else {
            "BLOCKED"
        };
        let label = format!("{prefix} {}", application.display_name.to_uppercase());
        draw_text(canvas, width, height, 20, y, 1, TEXT, &truncate(&label, 34));
        draw_text(
            canvas,
            width,
            height,
            width.saturating_sub(70),
            y,
            1,
            if application.launch_ready {
                READY
            } else {
                BLOCKED
            },
            state,
        );
    }

    if applications.is_empty() {
        draw_text(
            canvas,
            width,
            height,
            20,
            92,
            1,
            MUTED,
            "NO APPLICATIONS AVAILABLE",
        );
    }

    let footer_y = height.saturating_sub(74);
    draw_text(
        canvas,
        width,
        height,
        18,
        footer_y,
        1,
        ACCENT,
        "UP/DOWN SELECT   ENTER LAUNCH",
    );
    draw_text(
        canvas,
        width,
        height,
        18,
        footer_y.saturating_add(16),
        1,
        MUTED,
        "SETTINGS  POWER/RESTART  RECOVERY",
    );
    draw_text(
        canvas,
        width,
        height,
        18,
        footer_y.saturating_add(30),
        1,
        MUTED,
        "UNAVAILABLE ACTIONS STAY DISABLED",
    );
    if let Some(message) = message {
        draw_text(
            canvas,
            width,
            height,
            18,
            height.saturating_sub(22),
            1,
            TEXT,
            &truncate(&message.to_uppercase(), 45),
        );
    }
}

pub(crate) fn paint_quick_controls(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    lines: &[String],
    power_ready: bool,
    pending_power: Option<SystemPowerAction>,
    message: Option<&str>,
) {
    fill_rect(canvas, width, height, 0, 0, width, height, PANEL_ALT);
    stroke_rect(canvas, width, height, 0, 0, width, height, BORDER);
    draw_text(
        canvas,
        width,
        height,
        18,
        18,
        2,
        TEXT,
        "QUICK CONTROLS / SETTINGS",
    );
    draw_text(
        canvas,
        width,
        height,
        20,
        50,
        1,
        MUTED,
        "PRIME SYSTEM TRUTH",
    );

    let reboot_y = height.saturating_sub(QUICK_REBOOT_OFFSET);
    let poweroff_y = height.saturating_sub(QUICK_POWEROFF_OFFSET);
    let mut y: u32 = 78;
    for line in lines {
        if y.saturating_add(18) >= reboot_y {
            break;
        }
        let blocked = line.contains("UNAVAILABLE");
        draw_text(
            canvas,
            width,
            height,
            20,
            y,
            1,
            if blocked { BLOCKED } else { TEXT },
            &truncate(&line.to_uppercase(), 40),
        );
        y += 18;
    }

    paint_power_action(
        canvas,
        width,
        height,
        PowerActionRow {
            y: reboot_y,
            label: "R  RESTART",
            ready: power_ready,
            pending: pending_power == Some(SystemPowerAction::Reboot),
        },
    );
    paint_power_action(
        canvas,
        width,
        height,
        PowerActionRow {
            y: poweroff_y,
            label: "P  POWER OFF",
            ready: power_ready,
            pending: pending_power == Some(SystemPowerAction::PowerOff),
        },
    );

    let footer = message.unwrap_or(if power_ready {
        "POWER ACTIONS REQUIRE DOUBLE CONFIRMATION"
    } else {
        "POWER MUTATION UNAVAILABLE"
    });
    draw_text(
        canvas,
        width,
        height,
        20,
        height.saturating_sub(24),
        1,
        if message.is_some() { TEXT } else { MUTED },
        &truncate(&footer.to_uppercase(), 40),
    );
}

struct PowerActionRow<'a> {
    y: u32,
    label: &'a str,
    ready: bool,
    pending: bool,
}

fn paint_power_action(canvas: &mut [u8], width: u32, height: u32, row: PowerActionRow<'_>) {
    let PowerActionRow {
        y,
        label,
        ready,
        pending,
    } = row;
    let color = if !ready {
        BLOCKED
    } else if pending {
        ACCENT
    } else {
        READY
    };
    if pending {
        fill_rect(
            canvas,
            width,
            height,
            12,
            y.saturating_sub(7),
            width.saturating_sub(24),
            QUICK_ACTION_HEIGHT,
            SELECTED,
        );
    }
    draw_text(canvas, width, height, 20, y, 1, color, label);
    draw_text(
        canvas,
        width,
        height,
        width.saturating_sub(76),
        y,
        1,
        color,
        if ready { "READY" } else { "BLOCKED" },
    );
}

pub(crate) fn quick_power_action_at(y: f64, height: u32) -> Option<SystemPowerAction> {
    let reboot_y = f64::from(height.saturating_sub(QUICK_REBOOT_OFFSET).saturating_sub(7));
    let poweroff_y = f64::from(
        height
            .saturating_sub(QUICK_POWEROFF_OFFSET)
            .saturating_sub(7),
    );
    let action_height = f64::from(QUICK_ACTION_HEIGHT);
    if (reboot_y..reboot_y + action_height).contains(&y) {
        Some(SystemPowerAction::Reboot)
    } else if (poweroff_y..poweroff_y + action_height).contains(&y) {
        Some(SystemPowerAction::PowerOff)
    } else {
        None
    }
}

pub(crate) fn orb_row_at(y: f64, application_count: usize) -> Option<usize> {
    if y < ORB_LIST_TOP {
        return None;
    }
    let index = ((y - ORB_LIST_TOP) / ORB_ROW_HEIGHT) as usize;
    (index < application_count).then_some(index)
}

fn truncate(value: &str, max_chars: usize) -> String {
    let character_count = value.chars().count();
    if character_count <= max_chars {
        return value.to_owned();
    }
    if max_chars < 3 {
        return value.chars().take(max_chars).collect();
    }
    let mut result = value.chars().take(max_chars - 3).collect::<String>();
    result.push_str("...");
    result
}

fn lift_rgb(argb: u32, lift: u8) -> u32 {
    let alpha = (argb >> 24) & 0xff;
    let red = (((argb >> 16) & 0xff) as u8).saturating_add(lift) as u32;
    let green = (((argb >> 8) & 0xff) as u8).saturating_add(lift) as u32;
    let blue = ((argb & 0xff) as u8).saturating_add(lift) as u32;
    (alpha << 24) | (red << 16) | (green << 8) | blue
}

#[expect(
    clippy::too_many_arguments,
    reason = "bounded software-raster primitive keeps canvas bounds, geometry and style explicit"
)]
fn fill_rect(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: u32,
) {
    let max_x = x.saturating_add(width).min(canvas_width);
    let max_y = y.saturating_add(height).min(canvas_height);
    for py in y.min(canvas_height)..max_y {
        for px in x.min(canvas_width)..max_x {
            set_pixel(canvas, canvas_width, px, py, color);
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "bounded software-raster primitive keeps canvas bounds, geometry and style explicit"
)]
fn stroke_rect(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: u32,
) {
    if width == 0 || height == 0 {
        return;
    }
    fill_rect(canvas, canvas_width, canvas_height, x, y, width, 1, color);
    fill_rect(
        canvas,
        canvas_width,
        canvas_height,
        x,
        y.saturating_add(height - 1),
        width,
        1,
        color,
    );
    fill_rect(canvas, canvas_width, canvas_height, x, y, 1, height, color);
    fill_rect(
        canvas,
        canvas_width,
        canvas_height,
        x.saturating_add(width - 1),
        y,
        1,
        height,
        color,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "bounded software-raster primitive keeps canvas bounds, geometry and style explicit"
)]
fn draw_text(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    x: u32,
    y: u32,
    scale: u32,
    color: u32,
    text: &str,
) {
    let mut cursor = x;
    for character in text.chars() {
        if character == '\n' {
            continue;
        }
        let glyph = glyph(character.to_ascii_uppercase());
        for (column, bits) in glyph.iter().enumerate() {
            for row in 0..7 {
                if bits & (1 << row) != 0 {
                    fill_rect(
                        canvas,
                        canvas_width,
                        canvas_height,
                        cursor + column as u32 * scale,
                        y + row * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
        cursor = cursor.saturating_add(6 * scale);
        if cursor >= canvas_width {
            break;
        }
    }
}

fn text_width(text: &str, scale: u32) -> u32 {
    text.chars().count() as u32 * 6 * scale
}

fn set_pixel(canvas: &mut [u8], width: u32, x: u32, y: u32, argb: u32) {
    let Some(offset) = (y as usize)
        .checked_mul(width as usize)
        .and_then(|value| value.checked_add(x as usize))
        .and_then(|value| value.checked_mul(4))
    else {
        return;
    };
    if offset + 4 <= canvas.len() {
        canvas[offset..offset + 4].copy_from_slice(&argb.to_le_bytes());
    }
}

fn glyph(character: char) -> [u8; 5] {
    match character {
        'A' => [0x7e, 0x09, 0x09, 0x09, 0x7e],
        'B' => [0x7f, 0x49, 0x49, 0x49, 0x36],
        'C' => [0x3e, 0x41, 0x41, 0x41, 0x22],
        'D' => [0x7f, 0x41, 0x41, 0x22, 0x1c],
        'E' => [0x7f, 0x49, 0x49, 0x49, 0x41],
        'F' => [0x7f, 0x09, 0x09, 0x09, 0x01],
        'G' => [0x3e, 0x41, 0x49, 0x49, 0x7a],
        'H' => [0x7f, 0x08, 0x08, 0x08, 0x7f],
        'I' => [0x41, 0x41, 0x7f, 0x41, 0x41],
        'J' => [0x20, 0x40, 0x41, 0x3f, 0x01],
        'K' => [0x7f, 0x08, 0x14, 0x22, 0x41],
        'L' => [0x7f, 0x40, 0x40, 0x40, 0x40],
        'M' => [0x7f, 0x02, 0x0c, 0x02, 0x7f],
        'N' => [0x7f, 0x04, 0x08, 0x10, 0x7f],
        'O' => [0x3e, 0x41, 0x41, 0x41, 0x3e],
        'P' => [0x7f, 0x09, 0x09, 0x09, 0x06],
        'Q' => [0x3e, 0x41, 0x51, 0x21, 0x5e],
        'R' => [0x7f, 0x09, 0x19, 0x29, 0x46],
        'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
        'T' => [0x01, 0x01, 0x7f, 0x01, 0x01],
        'U' => [0x3f, 0x40, 0x40, 0x40, 0x3f],
        'V' => [0x1f, 0x20, 0x40, 0x20, 0x1f],
        'W' => [0x7f, 0x20, 0x18, 0x20, 0x7f],
        'X' => [0x63, 0x14, 0x08, 0x14, 0x63],
        'Y' => [0x03, 0x04, 0x78, 0x04, 0x03],
        'Z' => [0x61, 0x51, 0x49, 0x45, 0x43],
        '0' => [0x3e, 0x51, 0x49, 0x45, 0x3e],
        '1' => [0x00, 0x42, 0x7f, 0x40, 0x00],
        '2' => [0x42, 0x61, 0x51, 0x49, 0x46],
        '3' => [0x21, 0x41, 0x45, 0x4b, 0x31],
        '4' => [0x18, 0x14, 0x12, 0x7f, 0x10],
        '5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        '6' => [0x3c, 0x4a, 0x49, 0x49, 0x30],
        '7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        '8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        '9' => [0x06, 0x49, 0x49, 0x29, 0x1e],
        '-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        '_' => [0x40, 0x40, 0x40, 0x40, 0x40],
        ':' => [0x00, 0x36, 0x36, 0x00, 0x00],
        '/' => [0x20, 0x10, 0x08, 0x04, 0x02],
        '.' => [0x00, 0x60, 0x60, 0x00, 0x00],
        '%' => [0x62, 0x64, 0x08, 0x13, 0x23],
        '[' => [0x00, 0x7f, 0x41, 0x41, 0x00],
        ']' => [0x00, 0x41, 0x41, 0x7f, 0x00],
        '(' => [0x00, 0x1c, 0x22, 0x41, 0x00],
        ')' => [0x00, 0x41, 0x22, 0x1c, 0x00],
        '>' => [0x08, 0x14, 0x22, 0x41, 0x00],
        '+' => [0x08, 0x08, 0x3e, 0x08, 0x08],
        ' ' => [0x00; 5],
        _ => [0x02, 0x01, 0x51, 0x09, 0x06],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orb_hit_testing_is_bounded() {
        assert_eq!(orb_row_at(ORB_LIST_TOP, 2), Some(0));
        assert_eq!(orb_row_at(ORB_LIST_TOP + ORB_ROW_HEIGHT, 2), Some(1));
        assert_eq!(orb_row_at(ORB_LIST_TOP - 1.0, 2), None);
        assert_eq!(orb_row_at(ORB_LIST_TOP + ORB_ROW_HEIGHT * 2.0, 2), None);
    }

    #[test]
    fn quick_power_hit_testing_maps_only_action_rows() {
        let height = 420;
        assert_eq!(
            quick_power_action_at(301.0, height),
            Some(SystemPowerAction::Reboot)
        );
        assert_eq!(
            quick_power_action_at(337.0, height),
            Some(SystemPowerAction::PowerOff)
        );
        assert_eq!(quick_power_action_at(280.0, height), None);
    }

    #[test]
    fn alpha_blend_preserves_opaque_destination() {
        let dst = Argb::from_u32(0xff050818);
        let src = Argb::from_u32(0x8022d3ee);
        let mixed = src.over(dst);
        assert_eq!(mixed.a, 255);
        assert!(mixed.g > dst.g);
        assert!(mixed.b > dst.b);
    }

    #[test]
    fn rect_geometry_contains_and_centers() {
        let rect = Rect::new(10, 20, 40, 60);
        assert!(rect.contains(10, 20));
        assert!(rect.contains(49, 79));
        assert!(!rect.contains(50, 80));
        assert_eq!(rect.center_x(), 30.0);
        assert_eq!(rect.center_y(), 50.0);
    }

    #[test]
    fn canvas_fill_clear_and_rounded_geometry_are_bounded() {
        let mut bytes = vec![0u8; 32 * 32 * 4];
        let mut canvas = Canvas::new(&mut bytes, 32, 32).unwrap();
        canvas.fill_rect(Rect::new(4, 4, 8, 8), Argb::from_u32(0xff22d3ee));
        assert_eq!(canvas.pixel(4, 4).unwrap(), Argb::from_u32(0xff22d3ee));
        assert_eq!(canvas.pixel(3, 3).unwrap(), Argb::TRANSPARENT);
        canvas.clear();
        assert_eq!(canvas.pixel(4, 4).unwrap(), Argb::TRANSPARENT);
        canvas.fill_rounded_rect(Rect::new(0, 0, 32, 32), 10, Argb::from_u32(0xcc0f172a));
        assert_eq!(canvas.pixel(0, 0).unwrap().a, 0);
        assert!(canvas.pixel(16, 16).unwrap().a > 0);
    }

    #[test]
    fn stroke_gradient_and_glow_have_distinct_material_behavior() {
        let mut bytes = vec![0u8; 64 * 64 * 4];
        let mut canvas = Canvas::new(&mut bytes, 64, 64).unwrap();
        canvas.stroke_rounded_rect(Rect::new(4, 4, 40, 40), 8, 2, Argb::from_u32(0xff8b5cf6));
        assert!(canvas.pixel(24, 4).unwrap().a > 0);
        assert_eq!(canvas.pixel(24, 24).unwrap().a, 0);
        canvas.vertical_gradient(
            Rect::new(48, 0, 8, 32),
            Argb::from_u32(0xff05050d),
            Argb::from_u32(0xff071021),
        );
        assert_ne!(canvas.pixel(50, 1).unwrap(), canvas.pixel(50, 30).unwrap());
        canvas.radial_glow(32.0, 52.0, 10.0, Argb::from_u32(0x8022d3ee));
        assert!(canvas.pixel(32, 52).unwrap().a > canvas.pixel(23, 52).unwrap().a);
    }

    #[test]
    fn circle_and_line_primitives_touch_expected_pixels_only() {
        let mut bytes = vec![0u8; 32 * 32 * 4];
        let mut canvas = Canvas::new(&mut bytes, 32, 32).unwrap();
        let color = Argb::from_u32(0xfff8fafc);
        canvas.circle(8, 8, 3, color);
        assert_eq!(canvas.pixel(8, 8).unwrap(), color);
        assert_eq!(canvas.pixel(0, 0).unwrap(), Argb::TRANSPARENT);
        canvas.line((16, 4), (16, 20), 1, color);
        assert_eq!(canvas.pixel(16, 12).unwrap(), color);
        assert_eq!(canvas.pixel(15, 12).unwrap(), Argb::TRANSPARENT);
    }

    #[test]
    fn prime_dark_theme_matches_brand_authority() {
        let theme = Theme::prime_dark();
        assert_eq!(theme.base_0, Argb::from_u32(0xff05050d));
        assert_eq!(theme.base_1, Argb::from_u32(0xff050818));
        assert_eq!(theme.base_2, Argb::from_u32(0xff071021));
        assert_eq!(theme.panel, Argb::from_u32(0xff0f172a));
        assert_eq!(theme.cyan, Argb::from_u32(0xff22d3ee));
        assert_eq!(theme.cyan_alt, Argb::from_u32(0xff06b6d4));
        assert_eq!(theme.violet, Argb::from_u32(0xff8b5cf6));
        assert_eq!(theme.violet_alt, Argb::from_u32(0xffa855f7));
        assert_eq!(theme.text, Argb::from_u32(0xfff8fafc));
        assert_eq!(theme.muted, Argb::from_u32(0xff94a3b8));
    }

    #[test]
    fn render_context_defaults_to_prime_dark_theme() {
        let context = RenderContext::default();
        assert_eq!(context.theme, Theme::prime_dark());
    }
}
