use prime_contracts::SystemPowerAction;

use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

pub(crate) const QUICK_WIDTH: u32 = 600;
pub(crate) const QUICK_HEIGHT: u32 = 902;
const TOP_CARD_GAP: u32 = 12;
const SUMMARY_GAP: u32 = 14;
pub(crate) const QUICK_GLASS_ALPHA: u8 = 48;

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
    pub(crate) collapse: Rect,
    pub(crate) restart: Rect,
    pub(crate) power_off: Rect,
}

impl QuickControlsLayout {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let action_y = height.saturating_sub(82) as i32;
        let action_width = width.saturating_sub(68) / 2;
        Self {
            bounds: Rect::new(0, 0, width, height),
            content: Rect::new(26, 92, width.saturating_sub(52), height.saturating_sub(206)),
            collapse: Rect::new(width.saturating_sub(142) as i32, 10, 130, 48),
            restart: Rect::new(26, action_y, action_width, 56),
            power_off: Rect::new((width / 2 + 8) as i32, action_y, action_width, 56),
        }
    }

    pub(crate) fn card_rect(self, index: usize) -> Rect {
        if index < 3 {
            let card_width = self.content.width.saturating_sub(TOP_CARD_GAP * 2) / 3;
            Rect::new(
                self.content.x + (index as u32 * (card_width + TOP_CARD_GAP)) as i32,
                self.content.y,
                card_width,
                82,
            )
        } else {
            let summary_width = self.content.width.saturating_sub(SUMMARY_GAP) / 2;
            let column = (index - 3).min(1) as u32;
            Rect::new(
                self.content.x + (column * (summary_width + SUMMARY_GAP)) as i32,
                self.content.y + 486,
                summary_width,
                138,
            )
        }
    }

    pub(crate) fn collapse_at(self, x: f64, y: f64) -> bool {
        self.collapse.contains(x.floor() as i32, y.floor() as i32)
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

fn paint_card(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    card: Rect,
    data: &QuickControlCard,
    alpha: u8,
) {
    canvas.fill_rounded_rect(
        card,
        16,
        Argb::from_u32(0xff0d1d31).with_alpha(((42u16 * alpha as u16) / 255) as u8),
    );
    canvas.stroke_rounded_rect(
        card,
        16,
        1,
        theme.text.with_alpha(((34u16 * alpha as u16) / 255) as u8),
    );
    draw_icon(
        canvas,
        Rect::new(card.x + 18, card.y + 21, 30, 30),
        data.icon,
        theme.cyan.with_alpha(alpha),
    );
    text.draw(
        canvas,
        (card.x + 60, card.y + 16),
        &data.label,
        TextStyle {
            size_px: 11,
            weight: FontWeight::Semibold,
        },
        theme.text.with_alpha(alpha),
    );
    text.draw(
        canvas,
        (card.x + 60, card.y + 43),
        &data.value,
        TextStyle {
            size_px: 11,
            weight: FontWeight::Regular,
        },
        theme.muted.with_alpha(alpha),
    );
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
    let slide = ((1.0 - progress) * 26.0).round() as i32;
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
        Argb::from_u32(0xff081727)
            .with_alpha(((u16::from(QUICK_GLASS_ALPHA) * alpha as u16) / 255) as u8),
    );
    canvas.stroke_rounded_rect(
        body,
        28,
        1,
        theme.text.with_alpha(((82u16 * alpha as u16) / 255) as u8),
    );
    canvas.radial_glow(
        canvas.width as f32 * 0.78,
        170.0 + slide as f32,
        260.0,
        theme.cyan.with_alpha(((52u16 * alpha as u16) / 255) as u8),
    );
    canvas.radial_glow(
        92.0,
        canvas.height as f32 * 0.70,
        250.0,
        theme
            .violet
            .with_alpha(((36u16 * alpha as u16) / 255) as u8),
    );

    text.draw(
        canvas,
        (26, 26 + slide),
        "QUICK CONTROLS",
        TextStyle {
            size_px: 14,
            weight: FontWeight::Semibold,
        },
        theme.text.with_alpha(alpha),
    );
    text.draw(
        canvas,
        (canvas.width as i32 - 92, 28 + slide),
        "Collapse",
        TextStyle {
            size_px: 11,
            weight: FontWeight::Regular,
        },
        theme.muted.with_alpha(alpha),
    );
    draw_icon(
        canvas,
        Rect::new(canvas.width as i32 - 32, 25 + slide, 16, 16),
        Icon::Chevron,
        theme.text.with_alpha(alpha),
    );

    let cards = lines
        .iter()
        .take(5)
        .map(|line| quick_control_card(line))
        .collect::<Vec<_>>();
    for (index, card_data) in cards.iter().enumerate().take(3) {
        let base = layout.card_rect(index);
        let card = Rect::new(base.x, base.y + slide, base.width, base.height);
        paint_card(canvas, text, theme, card, card_data, alpha);
    }

    let divider_y = layout.content.y + 112 + slide;
    canvas.fill_rect(
        Rect::new(26, divider_y, canvas.width.saturating_sub(52), 1),
        theme.text.with_alpha(((28u16 * alpha as u16) / 255) as u8),
    );

    text.draw(
        canvas,
        (26, divider_y + 28),
        "SYSTEM STATE",
        TextStyle {
            size_px: 13,
            weight: FontWeight::Semibold,
        },
        theme.text.with_alpha(alpha),
    );
    let state_box = Rect::new(26, divider_y + 56, canvas.width.saturating_sub(52), 170);
    canvas.fill_rounded_rect(
        state_box,
        16,
        Argb::from_u32(0xff0b1a2c).with_alpha(((44u16 * alpha as u16) / 255) as u8),
    );
    canvas.stroke_rounded_rect(
        state_box,
        16,
        1,
        theme.text.with_alpha(((32u16 * alpha as u16) / 255) as u8),
    );
    let mut state_y = state_box.y + 24;
    for card in cards.iter().take(3) {
        draw_icon(
            canvas,
            Rect::new(state_box.x + 18, state_y - 2, 20, 20),
            card.icon,
            theme.cyan.with_alpha(alpha),
        );
        text.draw(
            canvas,
            (state_box.x + 50, state_y),
            &card.label,
            TextStyle {
                size_px: 11,
                weight: FontWeight::Semibold,
            },
            theme.muted.with_alpha(alpha),
        );
        let value_x = state_box.x + 170;
        text.draw(
            canvas,
            (value_x, state_y),
            &card.value,
            TextStyle {
                size_px: 12,
                weight: FontWeight::Regular,
            },
            theme.text.with_alpha(alpha),
        );
        state_y += 46;
    }

    text.draw(
        canvas,
        (26, layout.content.y + 402 + slide),
        "STORAGE",
        TextStyle {
            size_px: 13,
            weight: FontWeight::Semibold,
        },
        theme.text.with_alpha(alpha),
    );
    text.draw(
        canvas,
        (canvas.width as i32 / 2 + 8, layout.content.y + 402 + slide),
        "HEALTH SUMMARY",
        TextStyle {
            size_px: 13,
            weight: FontWeight::Semibold,
        },
        theme.text.with_alpha(alpha),
    );

    for (index, data) in cards.iter().enumerate().take(5).skip(3) {
        let base = layout.card_rect(index);
        let card = Rect::new(base.x, base.y + slide, base.width, base.height);
        canvas.fill_rounded_rect(
            card,
            16,
            Argb::from_u32(0xff0d1d31).with_alpha(((44u16 * alpha as u16) / 255) as u8),
        );
        canvas.stroke_rounded_rect(
            card,
            16,
            1,
            theme.text.with_alpha(((38u16 * alpha as u16) / 255) as u8),
        );
        draw_icon(
            canvas,
            Rect::new(card.x + 20, card.y + 28, 40, 40),
            data.icon,
            if data.label == "HEALTH" {
                Argb::from_u32(0xff54efcf)
            } else {
                theme.cyan
            }
            .with_alpha(alpha),
        );
        text.draw(
            canvas,
            (card.x + 76, card.y + 26),
            &data.value,
            TextStyle {
                size_px: 17,
                weight: FontWeight::Semibold,
            },
            if data.label == "HEALTH" {
                Argb::from_u32(0xff54efcf)
            } else {
                theme.text
            }
            .with_alpha(alpha),
        );
        text.draw(
            canvas,
            (card.x + 76, card.y + 58),
            &data.label,
            TextStyle {
                size_px: 10,
                weight: FontWeight::Regular,
            },
            theme.muted.with_alpha(alpha),
        );
    }

    let action_header_y = layout.restart.y - 44 + slide;
    canvas.fill_rect(
        Rect::new(26, action_header_y - 16, canvas.width.saturating_sub(52), 1),
        theme.text.with_alpha(((28u16 * alpha as u16) / 255) as u8),
    );
    text.draw(
        canvas,
        (26, action_header_y),
        "SYSTEM ACTIONS",
        TextStyle {
            size_px: 13,
            weight: FontWeight::Semibold,
        },
        theme.text.with_alpha(alpha),
    );

    let action_style = TextStyle {
        size_px: 12,
        weight: FontWeight::Semibold,
    };
    paint_action(
        canvas,
        text,
        theme,
        layout.restart,
        "REBOOT",
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
            (26, layout.restart.y - 74 + slide),
            message,
            TextStyle {
                size_px: 11,
                weight: FontWeight::Regular,
            },
            theme.muted.with_alpha(alpha),
        );
    } else if !power_ready {
        text.draw(
            canvas,
            (26, layout.restart.y - 74 + slide),
            "Power mutation unavailable",
            TextStyle {
                size_px: 11,
                weight: FontWeight::Regular,
            },
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
        Argb::from_u32(0xff0b1a2c).with_alpha(((58u16 * alpha as u16) / 255) as u8)
    };
    canvas.fill_rounded_rect(rect, 14, fill);
    canvas.stroke_rounded_rect(
        rect,
        14,
        1,
        if pending {
            theme.cyan.with_alpha(alpha)
        } else {
            theme.text.with_alpha(((38u16 * alpha as u16) / 255) as u8)
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
        Rect::new(rect.x + 18, rect.y + 18, 20, 20),
        icon,
        if label == "POWER OFF" && ready {
            Argb::from_u32(0xffff605c).with_alpha(alpha)
        } else {
            color
        },
    );
    text.draw(canvas, (rect.x + 52, rect.y + 18), label, style, color);
}
