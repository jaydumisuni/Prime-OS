use prime_contracts::SystemPowerAction;

use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

pub(crate) const QUICK_WIDTH: u32 = 430;
pub(crate) const QUICK_HEIGHT: u32 = 600;

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
        let action_y = height.saturating_sub(82) as i32;
        let action_width = width.saturating_sub(60) / 2;
        Self {
            bounds: Rect::new(0, 0, width, height),
            content: Rect::new(24, 82, width.saturating_sub(48), height.saturating_sub(184)),
            restart: Rect::new(24, action_y, action_width, 54),
            power_off: Rect::new((width / 2 + 6) as i32, action_y, action_width, 54),
        }
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
        "QUICK CONTROLS",
        heading,
        theme.text.with_alpha(alpha),
    );
    text.draw(
        canvas,
        (24, 57 + slide),
        "Prime system truth",
        secondary,
        theme.muted.with_alpha(alpha),
    );
    draw_icon(
        canvas,
        Rect::new(canvas.width as i32 - 48, 25 + slide, 18, 18),
        Icon::Chevron,
        theme.muted.with_alpha(alpha),
    );

    let card_width = layout.content.width;
    let card_height = 56u32;
    let pitch = 64u32;
    let max_rows = (layout.content.height / pitch).min(6) as usize;
    for (index, line) in lines.iter().take(max_rows).enumerate() {
        let y = layout.content.y + (index as u32 * pitch) as i32 + slide;
        let card = Rect::new(layout.content.x, y, card_width, card_height);
        canvas.fill_rounded_rect(
            card,
            16,
            Argb::from_u32(0xff0b1222).with_alpha(((148u16 * alpha as u16) / 255) as u8),
        );
        canvas.stroke_rounded_rect(
            card,
            16,
            1,
            theme.text.with_alpha(((28u16 * alpha as u16) / 255) as u8),
        );
        let icon = if line.to_ascii_uppercase().contains("STORAGE") {
            Icon::Storage
        } else if line.to_ascii_uppercase().contains("NETWORK")
            || line.to_ascii_uppercase().contains("WIFI")
        {
            Icon::Network
        } else if line.to_ascii_uppercase().contains("AUDIO")
            || line.to_ascii_uppercase().contains("VOLUME")
        {
            Icon::Audio
        } else if line.to_ascii_uppercase().contains("HEALTH") {
            Icon::Health
        } else {
            Icon::Status
        };
        draw_icon(
            canvas,
            Rect::new(card.x + 14, card.y + 17, 22, 22),
            icon,
            theme.cyan.with_alpha(alpha),
        );
        text.draw(
            canvas,
            (card.x + 50, card.y + 17),
            line,
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
