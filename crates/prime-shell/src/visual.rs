use prime_contracts::ApplicationEntry;

pub(crate) const ORB_LIST_TOP: f64 = 82.0;
pub(crate) const ORB_ROW_HEIGHT: f64 = 30.0;

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
    fill_rect(canvas, width, height, 0, height.saturating_sub(1), width, 1, BORDER);
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
    draw_text(canvas, width, height, 20, 52, 1, MUTED, "ADMITTED APPLICATIONS");

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
            if application.launch_ready { READY } else { BLOCKED },
            state,
        );
    }

    if applications.is_empty() {
        draw_text(canvas, width, height, 20, 92, 1, MUTED, "NO APPLICATIONS AVAILABLE");
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
) {
    fill_rect(canvas, width, height, 0, 0, width, height, PANEL_ALT);
    stroke_rect(canvas, width, height, 0, 0, width, height, BORDER);
    draw_text(canvas, width, height, 18, 18, 2, TEXT, "QUICK CONTROLS");
    draw_text(canvas, width, height, 20, 50, 1, MUTED, "PRIME SYSTEM TRUTH");

    let mut y = 78;
    for line in lines.iter().take(13) {
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
    draw_text(
        canvas,
        width,
        height,
        20,
        height.saturating_sub(24),
        1,
        MUTED,
        "READ-ONLY UNTIL MUTATION BACKENDS EARNED",
    );
}

pub(crate) fn orb_row_at(y: f64, application_count: usize) -> Option<usize> {
    if y < ORB_LIST_TOP {
        return None;
    }
    let index = ((y - ORB_LIST_TOP) / ORB_ROW_HEIGHT) as usize;
    (index < application_count).then_some(index)
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut result = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() && max_chars >= 3 {
        result.truncate(result.len().saturating_sub(3));
        result.push_str("...");
    }
    result
}

fn lift_rgb(argb: u32, lift: u8) -> u32 {
    let alpha = (argb >> 24) & 0xff;
    let red = (((argb >> 16) & 0xff) as u8).saturating_add(lift) as u32;
    let green = (((argb >> 8) & 0xff) as u8).saturating_add(lift) as u32;
    let blue = ((argb & 0xff) as u8).saturating_add(lift) as u32;
    (alpha << 24) | (red << 16) | (green << 8) | blue
}

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
}
