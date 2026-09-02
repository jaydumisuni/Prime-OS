use prime_contracts::ApplicationEntry;

use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

pub(crate) const PRIME_LAUNCHER_WIDTH: u32 = 520;
pub(crate) const PRIME_LAUNCHER_HEIGHT: u32 = 520;
const CARD_COLUMNS: u32 = 2;
const CARD_GAP: u32 = 12;
const CARD_HEIGHT: u32 = 88;
const CARD_ROW_GAP: u32 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PrimeLauncherLayout {
    pub(crate) bounds: Rect,
    pub(crate) search: Rect,
    pub(crate) apps: Rect,
    pub(crate) footer: Rect,
}

impl PrimeLauncherLayout {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let bounds = Rect::new(0, 0, width, height);
        let search = Rect::new(24, 24, width.saturating_sub(48), 52);
        let apps_y = 116;
        let footer_height = 54;
        let apps_height = height.saturating_sub(apps_y + footer_height + 22);
        let apps = Rect::new(24, apps_y as i32, width.saturating_sub(48), apps_height);
        let footer = Rect::new(
            24,
            height.saturating_sub(footer_height + 12) as i32,
            width.saturating_sub(48),
            footer_height,
        );
        Self {
            bounds,
            search,
            apps,
            footer,
        }
    }

    pub(crate) fn card_rect(self, index: usize) -> Rect {
        let card_width = self.apps.width.saturating_sub(CARD_GAP) / CARD_COLUMNS;
        let column = index as u32 % CARD_COLUMNS;
        let row = index as u32 / CARD_COLUMNS;
        Rect::new(
            self.apps.x + (column * (card_width + CARD_GAP)) as i32,
            self.apps.y + (row * (CARD_HEIGHT + CARD_ROW_GAP)) as i32,
            card_width,
            CARD_HEIGHT,
        )
    }

    pub(crate) fn application_at(self, x: f64, y: f64, count: usize) -> Option<usize> {
        (0..count.min(6)).find(|&index| {
            self.card_rect(index)
                .contains(x.floor() as i32, y.floor() as i32)
        })
    }
}

pub(crate) const fn application_state_label(launch_ready: bool) -> Option<&'static str> {
    if launch_ready {
        None
    } else {
        Some("Unavailable")
    }
}

pub(crate) fn paint_prime_launcher_surface(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    applications: &[ApplicationEntry],
    selected: usize,
    message: Option<&str>,
    progress: f32,
) {
    canvas.clear();
    let progress = progress.clamp(0.0, 1.0);
    let layout = PrimeLauncherLayout::new(canvas.width, canvas.height);
    let slide = ((1.0 - progress) * 28.0).round() as i32;
    let alpha = (progress * 255.0).round() as u8;
    let body = Rect::new(
        1 + slide,
        1,
        canvas.width.saturating_sub(2 + slide.max(0) as u32),
        canvas.height.saturating_sub(2),
    );
    canvas.fill_rounded_rect(
        body,
        28,
        theme
            .panel
            .with_alpha(((178u16 * alpha as u16) / 255) as u8),
    );
    canvas.stroke_rounded_rect(
        body,
        28,
        1,
        theme.text.with_alpha(((68u16 * alpha as u16) / 255) as u8),
    );
    canvas.radial_glow(
        90.0 + slide as f32,
        86.0,
        140.0,
        theme
            .violet
            .with_alpha(((82u16 * alpha as u16) / 255) as u8),
    );
    canvas.radial_glow(
        canvas.width as f32 * 0.78,
        180.0,
        160.0,
        theme.cyan.with_alpha(((44u16 * alpha as u16) / 255) as u8),
    );

    let shifted = |rect: Rect| {
        Rect::new(
            rect.x + slide,
            rect.y,
            rect.width.saturating_sub(slide.max(0) as u32),
            rect.height,
        )
    };
    let search = shifted(layout.search);
    canvas.fill_rounded_rect(
        search,
        16,
        Argb::from_u32(0xff071021).with_alpha(((174u16 * alpha as u16) / 255) as u8),
    );
    canvas.stroke_rounded_rect(
        search,
        16,
        1,
        theme.cyan.with_alpha(((62u16 * alpha as u16) / 255) as u8),
    );
    draw_icon(
        canvas,
        Rect::new(search.x + 16, search.y + 15, 22, 22),
        Icon::Search,
        theme.muted.with_alpha(alpha),
    );
    let search_style = TextStyle::body();
    text.draw(
        canvas,
        (search.x + 50, search.y + 14),
        "Search apps and commands",
        search_style,
        theme.muted.with_alpha(alpha),
    );

    let section_style = TextStyle {
        size_px: 10,
        weight: FontWeight::Semibold,
    };
    text.draw(
        canvas,
        (layout.apps.x + slide, 92),
        "APPS",
        section_style,
        theme.muted.with_alpha(alpha),
    );
    for (index, application) in applications.iter().take(6).enumerate() {
        let row = shifted(layout.card_rect(index));
        let selected_row = index == selected;
        canvas.fill_rounded_rect(
            row,
            18,
            if selected_row {
                theme
                    .violet
                    .with_alpha(((52u16 * alpha as u16) / 255) as u8)
            } else {
                Argb::from_u32(0xff0b1222).with_alpha(((142u16 * alpha as u16) / 255) as u8)
            },
        );
        canvas.stroke_rounded_rect(
            row,
            18,
            1,
            if selected_row {
                theme.cyan.with_alpha(((142u16 * alpha as u16) / 255) as u8)
            } else {
                theme.text.with_alpha(((32u16 * alpha as u16) / 255) as u8)
            },
        );
        let icon_rect = Rect::new(row.x + 18, row.y + 20, 32, 32);
        draw_icon(
            canvas,
            icon_rect,
            Icon::Applications,
            if selected_row {
                theme.cyan.with_alpha(alpha)
            } else {
                theme.violet.with_alpha(alpha)
            },
        );
        let title = TextStyle {
            size_px: 15,
            weight: FontWeight::Semibold,
        };
        text.draw(
            canvas,
            (row.x + 66, row.y + 15),
            &application.display_name,
            title,
            theme.text.with_alpha(alpha),
        );
        if let Some(state) = application_state_label(application.launch_ready) {
            let state_color = Argb::from_u32(0xfff59e0b);
            text.draw(
                canvas,
                (row.x + 66, row.y + 43),
                state,
                section_style,
                state_color.with_alpha(alpha),
            );
            draw_icon(
                canvas,
                Rect::new(row.x + row.width as i32 - 34, row.y + 25, 16, 16),
                Icon::Blocked,
                state_color.with_alpha(alpha),
            );
        }
    }

    if applications.is_empty() {
        let empty = shifted(Rect::new(
            layout.apps.x,
            layout.apps.y,
            layout.apps.width,
            92,
        ));
        canvas.fill_rounded_rect(
            empty,
            18,
            Argb::from_u32(0xff0b1222).with_alpha(((142u16 * alpha as u16) / 255) as u8),
        );
        text.draw(
            canvas,
            (empty.x + 20, empty.y + 28),
            "No admitted applications",
            search_style,
            theme.muted.with_alpha(alpha),
        );
    }

    let footer = shifted(layout.footer);
    canvas.fill_rounded_rect(
        footer,
        16,
        Argb::from_u32(0xff071021).with_alpha(((148u16 * alpha as u16) / 255) as u8),
    );
    let footer_text = message.unwrap_or("Select an app to open");
    text.draw(
        canvas,
        (footer.x + 16, footer.y + 16),
        footer_text,
        TextStyle {
            size_px: 11,
            weight: FontWeight::Regular,
        },
        theme.muted.with_alpha(alpha),
    );
}
