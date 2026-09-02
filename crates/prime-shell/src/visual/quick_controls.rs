use prime_contracts::SystemPowerAction;

use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

pub(crate) const QUICK_WIDTH: u32 = 430;
pub(crate) const QUICK_HEIGHT: u32 = 560;
const CARD_COLUMNS: u32 = 2;
const CARD_GAP: u32 = 12;
const CARD_HEIGHT: u32 = 86;
const CARD_ROW_GAP: u32 = 12;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuickControlCard {
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) icon: Icon,
}

pub(crate) fn quick_control_card(line: &str) -> QuickControlCard {
    let trimmed = line.trim();
    let upper = trimmed.to_ascii_uppercase();
    let (label, icon, value) = if let Some(rest) = upper
        .strip_prefix("NET ")
        .or_else(|| upper.strip_prefix("NETWORK "))
        .or_else(|| upper.strip_prefix("WIFI "))
    {
        let original_offset = trimmed.len().saturating_sub(rest.len());
        ("NETWORK", Icon::Network, trimmed[original_offset..].trim())
    } else if let Some(rest) = upper.strip_prefix("AUDIO ") {
        let original_offset = trimmed.len().saturating_sub(rest.len());
        ("AUDIO", Icon::Audio, trimmed[original_offset..].trim())
    } else if let Some(rest) = upper.strip_prefix("STORAGE ") {
        let original_offset = trimmed.len().saturating_sub(rest.len());
        ("STORAGE", Icon::Storage, trimmed[original_offset..].trim())
    } else if upper.starts_with("HEALTH") {
        let value = trimmed
            .split_once(':')
            .map(|(_, value)| value.trim())
            .unwrap_or(trimmed);
        ("HEALTH", Icon::Health, value)
    } else if upper.starts_with("PWR") || upper.starts_with("POWER") {
        let value = trimmed
            .split_once(' ')
            .map(|(_, value)| value.trim())
            .unwrap_or(trimmed);
        ("POWER", Icon::Status, value)
    } else {
        let (label, value) = trimmed
            .split_once(':')
            .map(|(label, value)| (label.trim(), value.trim()))
            .unwrap_or(("STATUS", trimmed));
        (label, Icon::Status, value)
    };
    QuickControlCard {
        label: label.to_owned(),
        value: value.to_owned(),
        icon,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct QuickControlsView<'a> {
    pub(crate) lines: &'a [String],
    pub(crate) power_ready: bool,
    pub(crate) pending_power: Option<SystemPowerAction>,
    pub(crate) message: Option<&'a str>,
    pub(crate) progress: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QuickControlsLayout {
    pub(crate) bounds: Rect,
    pub(crate) content: Rect,
    pub(crate) restart: Rect,
    pub(crate) power_off: Rect,
}

impl QuickControlsLayout {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let action_y = height.saturating_sub(78) as i32;
        let action_width = width.saturating_sub(60) / 2;
        Self {
            bounds: Rect::new(0, 0, width, height),
            content: Rect::new(24, 90, width.saturating_sub(48), height.saturating_sub(190)),
            restart: Rect::new(24, action_y, action_width, 50),
            power_off: Rect::new((width / 2 + 6) as i32, action_y, action_width, 50),
        }
    }

    pub(crate) fn card_rect(self, index: usize) -> Rect {
        let card_width = self.content.width.saturating_sub(CARD_GAP) / CARD_COLUMNS;
        let column = index as u32 % CARD_COLUMNS;
        let row = index as u32 / CARD_COLUMNS;
        Rect::new(
            self.content.x + (column * (card_width + CARD_GAP)) as i32,
            self.content.y + (row * (CARD_HEIGHT + CARD_ROW_GAP)) as i32,
            card_width,
            CARD_HEIGHT,
        )
    }

    pub(crate) fn power_action_at(self, x: f64, y: f64) -> Option<SystemPowerAction> {
        let x = x.floor() as i32;
        let y = y.floor() as i32;
        if self.restart.contains(x, y) {
            Some(SystemPowerAction::Reboot)
        } else if self.power_off.contains(x, y) {
            Some(SystemPowerAction::PowerOff)
        } else {
            None
        }
    }
}

pub(crate) fn paint_quick_controls_surface(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    view: QuickControlsView<'_>,
) {
    canvas.clear();
    let QuickControlsView {
        lines,
        power_ready,
        pending_power,
        message,
        progress,
    } = view;
    let progress = progress.clamp(0.0, 1.0);
    let alpha = (progress * 255.0).round() as u8;
    let slide = ((1.0 - progress) * 24.0).round() as i32;
    let layout = QuickControlsLayout::new(canvas.width, canvas.height);
    let body = Rect::new(
        1,
        1 + slide,
        canvas.width.saturating_sub(2),
        canvas.height.saturating_sub(2 + slide.max(0) as u32),
    );
    canvas.fill_rounded_rect(
        body,
        28,
        theme
            .panel
            .with_alpha(((182u16 * alpha as u16) / 255) as u8),
    );
    canvas.stroke_rounded_rect(
        body,
        28,
        1,
        theme.text.with_alpha(((70u16 * alpha as u16) / 255) as u8),
    );
    canvas.radial_glow(
        canvas.width as f32 * 0.80,
        82.0 + slide as f32,
        130.0,
        theme.cyan.with_alpha(((54u16 * alpha as u16) / 255) as u8),
    );
    canvas.radial_glow(
        80.0,
        canvas.height as f32 * 0.62,
        160.0,
        theme
            .violet
            .with_alpha(((48u16 * alpha as u16) / 255) as u8),
    );

    let heading = TextStyle::title();
    let secondary = TextStyle {
        size_px: 11,
        weight: FontWeight::Regular,
    };
    text.draw(
        canvas,
        (24, 24 + slide),
        "Quick Controls",
        heading,
        theme.text.with_alpha(alpha),
    );
    text.draw(
        canvas,
        (24, 57 + slide),
        "System controls and status",
        secondary,
        theme.muted.with_alpha(alpha),
    );
    draw_icon(
        canvas,
        Rect::new(canvas.width as i32 - 48, 25 + slide, 18, 18),
        Icon::Chevron,
        theme.muted.with_alpha(alpha),
    );

    for (index, line) in lines.iter().take(6).enumerate() {
        let card_data = quick_control_card(line);
        let base = layout.card_rect(index);
        let card = Rect::new(base.x, base.y + slide, base.width, base.height);
        canvas.fill_rounded_rect(
            card,
            18,
            Argb::from_u32(0xff0b1222).with_alpha(((142u16 * alpha as u16) / 255) as u8),
        );
        canvas.stroke_rounded_rect(
            card,
            18,
            1,
            theme.text.with_alpha(((30u16 * alpha as u16) / 255) as u8),
        );
        draw_icon(
            canvas,
            Rect::new(card.x + 14, card.y + 14, 22, 22),
            card_data.icon,
            theme.cyan.with_alpha(alpha),
        );
        text.draw(
            canvas,
            (card.x + 46, card.y + 13),
            &card_data.label,
            TextStyle {
                size_px: 10,
                weight: FontWeight::Semibold,
            },
            theme.muted.with_alpha(alpha),
        );
        text.draw(
            canvas,
            (card.x + 14, card.y + 48),
            &card_data.value,
            secondary,
            theme.text.with_alpha(alpha),
        );
    }

    let action_style = TextStyle {
        size_px: 12,
        weight: FontWeight::Semibold,
    };
    paint_action(
        canvas,
        text,
        theme,
        layout.restart,
        "RESTART",
        Icon::Restart,
        power_ready,
        pending_power == Some(SystemPowerAction::Reboot),
        alpha,
        slide,
        action_style,
    );
    paint_action(
        canvas,
        text,
        theme,
        layout.power_off,
        "POWER OFF",
        Icon::Power,
        power_ready,
        pending_power == Some(SystemPowerAction::PowerOff),
        alpha,
        slide,
        action_style,
    );

    if let Some(message) = message {
        text.draw(
            canvas,
            (24, canvas.height as i32 - 110 + slide),
            message,
            secondary,
            theme.muted.with_alpha(alpha),
        );
    } else if !power_ready {
        text.draw(
            canvas,
            (24, canvas.height as i32 - 110 + slide),
            "Power mutation unavailable",
            secondary,
            Argb::from_u32(0xfff59e0b).with_alpha(alpha),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_action(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    rect: Rect,
    label: &str,
    icon: Icon,
    ready: bool,
    pending: bool,
    alpha: u8,
    slide: i32,
    style: TextStyle,
) {
    let rect = Rect::new(rect.x, rect.y + slide, rect.width, rect.height);
    let fill = if pending {
        theme
            .violet
            .with_alpha(((76u16 * alpha as u16) / 255) as u8)
    } else {
        Argb::from_u32(0xff0b1222).with_alpha(((150u16 * alpha as u16) / 255) as u8)
    };
    canvas.fill_rounded_rect(rect, 16, fill);
    canvas.stroke_rounded_rect(
        rect,
        16,
        1,
        if pending {
            theme.cyan.with_alpha(alpha)
        } else {
            theme.text.with_alpha(((36u16 * alpha as u16) / 255) as u8)
        },
    );
    let color = if ready {
        theme.text
    } else {
        Argb::from_u32(0xff64748b)
    }
    .with_alpha(alpha);
    draw_icon(
        canvas,
        Rect::new(rect.x + 14, rect.y + 17, 20, 20),
        icon,
        color,
    );
    text.draw(canvas, (rect.x + 44, rect.y + 17), label, style, color);
}
