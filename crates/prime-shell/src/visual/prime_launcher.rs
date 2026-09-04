use prime_contracts::{ApplicationEntry, SystemPowerAction};

use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

pub(crate) const PRIME_LAUNCHER_WIDTH: u32 = 858;
pub(crate) const PRIME_LAUNCHER_HEIGHT: u32 = 786;
pub(crate) const PRIME_LAUNCHER_TOP_MARGIN: i32 = 140;
pub(crate) const PRIME_LAUNCHER_LEFT_MARGIN: i32 = 210;
const CARD_COLUMNS: u32 = 4;
const CARD_GAP: u32 = 18;
const CARD_HEIGHT: u32 = 208;
const CARD_ROW_GAP: u32 = 18;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PrimeLauncherLayout {
    pub(crate) bounds: Rect,
    pub(crate) search: Rect,
    pub(crate) apps: Rect,
    pub(crate) footer: Rect,
    pub(crate) lock: Rect,
    pub(crate) log_out: Rect,
    pub(crate) restart: Rect,
    pub(crate) power_off: Rect,
}

impl PrimeLauncherLayout {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let bounds = Rect::new(0, 0, width, height);
        let search = Rect::new(42, 34, width.saturating_sub(84), 64);
        let apps_y = 136;
        let footer_height = 50;
        let apps_height = height.saturating_sub(apps_y + footer_height + 20);
        let apps = Rect::new(42, apps_y as i32, width.saturating_sub(84), apps_height);
        let footer = Rect::new(
            42,
            height.saturating_sub(44) as i32,
            width.saturating_sub(84),
            18,
        );
        let actions_y = height.saturating_sub(112) as i32;
        let actions_width = width.saturating_sub(84);
        let action_gap = 14;
        let action_width = actions_width.saturating_sub(action_gap * 3) / 4;
        let action = |column: u32| {
            Rect::new(
                42 + (column * (action_width + action_gap)) as i32,
                actions_y,
                action_width,
                48,
            )
        };
        Self {
            bounds,
            search,
            apps,
            footer,
            lock: action(0),
            log_out: action(1),
            restart: action(2),
            power_off: action(3),
        }
    }

    pub(crate) fn card_rect(self, index: usize) -> Rect {
        let card_width = self
            .apps
            .width
            .saturating_sub(CARD_GAP * (CARD_COLUMNS - 1))
            / CARD_COLUMNS;
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
        (0..count.min(8)).find(|&index| {
            self.card_rect(index)
                .contains(x.floor() as i32, y.floor() as i32)
        })
    }

    pub(crate) fn power_action_at(self, _x: f64, _y: f64) -> Option<SystemPowerAction> {
        // System power belongs exclusively to Quick Controls. Keeping Home
        // non-mutating avoids duplicate actions and a second confirmation path.
        None
    }
}

pub(crate) const fn application_state_label(launch_ready: bool) -> Option<&'static str> {
    if launch_ready { None } else { Some("Unavailable") }
}

#[derive(Clone, Copy)]
pub(crate) struct PrimeLauncherView<'a> {
    pub(crate) applications: &'a [ApplicationEntry],
    pub(crate) selected: usize,
    pub(crate) power_ready: bool,
    pub(crate) pending_power: Option<SystemPowerAction>,
    pub(crate) message: Option<&'a str>,
    pub(crate) progress: f32,
}

fn application_presentation(name: &str) -> (Icon, &'static str, &'static str) {
    let lower = name.to_ascii_lowercase();
    if lower.contains("file") {
        (Icon::Files, "Browse and manage", "system files")
    } else if lower.contains("terminal") {
        (Icon::Terminal, "Command line", "interface")
    } else if lower.contains("diagnostic") || lower.contains("health") {
        (Icon::Health, "System health", "and diagnostics")
    } else if lower.contains("setting") {
        (Icon::Settings, "System preferences", "and customization")
    } else if lower.contains("network") || lower.contains("wifi") {
        (Icon::Wifi, "Connections and", "network settings")
    } else if lower.contains("browser") || lower.contains("web") {
        (Icon::Browser, "Secure web", "experience")
    } else if lower.contains("media") || lower.contains("player") {
        (Icon::Media, "Media player and", "content hub")
    } else if lower.contains("recovery") || lower.contains("backup") {
        (Icon::Recovery, "Backup, restore,", "and recovery")
    } else {
        (Icon::Applications, "Native Prime", "application")
    }
}

fn centered_text_x(text: &TextSystem, label: &str, style: TextStyle, rect: Rect) -> i32 {
    rect.center_x().round() as i32 - text.measure(label, style).width as i32 / 2
}

pub(crate) fn paint_prime_launcher_surface(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    view: PrimeLauncherView<'_>,
) {
    canvas.clear();
    let PrimeLauncherView {
        applications,
        selected,
        power_ready: _,
        pending_power: _,
        message,
        progress,
    } = view;
    let progress = progress.clamp(0.0, 1.0);
    let layout = PrimeLauncherLayout::new(canvas.width, canvas.height);
    let slide = ((1.0 - progress) * 30.0).round() as i32;
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
        Argb::from_u32(0xff0a1326).with_alpha(((50u16 * alpha as u16) / 255) as u8),
    );
    canvas.stroke_rounded_rect(
        body,
        28,
        1,
        theme.cyan.with_alpha(((78u16 * alpha as u16) / 255) as u8),
    );
    canvas.stroke_rounded_rect(
        Rect::new(
            body.x + 2,
            body.y + 2,
            body.width.saturating_sub(4),
            body.height.saturating_sub(4),
        ),
        27,
        1,
        theme.text.with_alpha(((42u16 * alpha as u16) / 255) as u8),
    );
    canvas.radial_glow(
        150.0 + slide as f32,
        canvas.height as f32 * 0.68,
        290.0,
        theme.violet.with_alpha(((34u16 * alpha as u16) / 255) as u8),
    );
    canvas.radial_glow(
        canvas.width as f32 * 0.82,
        210.0,
        270.0,
        theme.cyan.with_alpha(((28u16 * alpha as u16) / 255) as u8),
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
        18,
        Argb::from_u32(0xff0b1529).with_alpha(((54u16 * alpha as u16) / 255) as u8),
    );
    canvas.stroke_rounded_rect(
        search,
        18,
        1,
        theme.text.with_alpha(((52u16 * alpha as u16) / 255) as u8),
    );
    draw_icon(
        canvas,
        Rect::new(search.x + 20, search.y + 20, 24, 24),
        Icon::Search,
        theme.text.with_alpha(((220u16 * alpha as u16) / 255) as u8),
    );
    text.draw(
        canvas,
        (search.x + 60, search.y + 20),
        "Search apps, files, and commands...",
        TextStyle { size_px: 14, weight: FontWeight::Regular },
        theme.muted.with_alpha(alpha),
    );

    text.draw(
        canvas,
        (layout.apps.x + slide, 108),
        "CORE APPS",
        TextStyle { size_px: 13, weight: FontWeight::Semibold },
        theme.text.with_alpha(((220u16 * alpha as u16) / 255) as u8),
    );

    for (index, application) in applications.iter().take(8).enumerate() {
        let card = shifted(layout.card_rect(index));
        let selected_card = index == selected;
        let fill = if selected_card {
            theme.violet.with_alpha(((44u16 * alpha as u16) / 255) as u8)
        } else {
            Argb::from_u32(0xff10203a).with_alpha(((48u16 * alpha as u16) / 255) as u8)
        };
        canvas.fill_rounded_rect(card, 18, fill);
        canvas.stroke_rounded_rect(
            card,
            18,
            1,
            if selected_card {
                theme.cyan.with_alpha(((126u16 * alpha as u16) / 255) as u8)
            } else {
                theme.text.with_alpha(((40u16 * alpha as u16) / 255) as u8)
            },
        );
        let (icon, desc1, desc2) = application_presentation(&application.display_name);
        let icon_color = if selected_card {
            theme.cyan
        } else if index % 3 == 1 {
            theme.violet
        } else {
            theme.cyan_alt
        };
        let icon_rect = Rect::new(card.center_x().round() as i32 - 27, card.y + 28, 54, 54);
        canvas.radial_glow(
            icon_rect.center_x() as f32,
            icon_rect.center_y() as f32,
            46.0,
            icon_color.with_alpha(((28u16 * alpha as u16) / 255) as u8),
        );
        draw_icon(canvas, icon_rect, icon, icon_color.with_alpha(alpha));
        let title_style = TextStyle { size_px: 15, weight: FontWeight::Semibold };
        text.draw(
            canvas,
            (centered_text_x(text, &application.display_name, title_style, card), card.y + 101),
            &application.display_name,
            title_style,
            theme.text.with_alpha(alpha),
        );
        let desc_style = TextStyle { size_px: 12, weight: FontWeight::Regular };
        text.draw(
            canvas,
            (centered_text_x(text, desc1, desc_style, card), card.y + 134),
            desc1,
            desc_style,
            theme.muted.with_alpha(alpha),
        );
        text.draw(
            canvas,
            (centered_text_x(text, desc2, desc_style, card), card.y + 154),
            desc2,
            desc_style,
            theme.muted.with_alpha(alpha),
        );
        if let Some(state) = application_state_label(application.launch_ready) {
            let state_style = TextStyle { size_px: 10, weight: FontWeight::Semibold };
            text.draw(
                canvas,
                (centered_text_x(text, state, state_style, card), card.y + 181),
                state,
                state_style,
                Argb::from_u32(0xfff59e0b).with_alpha(alpha),
            );
        }
    }

    if applications.is_empty() {
        let empty = shifted(Rect::new(layout.apps.x, layout.apps.y, layout.apps.width, 116));
        canvas.fill_rounded_rect(
            empty,
            18,
            Argb::from_u32(0xff0b1222).with_alpha(((50u16 * alpha as u16) / 255) as u8),
        );
        text.draw(
            canvas,
            (empty.x + 24, empty.y + 42),
            "No admitted applications",
            TextStyle::body(),
            theme.muted.with_alpha(alpha),
        );
    }

    if let Some(footer_text) = message {
        let footer = Rect::new(
            layout.footer.x + slide,
            layout.footer.y,
            layout.footer.width.saturating_sub(slide.max(0) as u32),
            layout.footer.height,
        );
        text.draw(
            canvas,
            (footer.x, footer.y),
            footer_text,
            TextStyle { size_px: 10, weight: FontWeight::Regular },
            theme.muted.with_alpha(alpha),
        );
    }
}

#[cfg(test)]
mod final_ui_contract_tests {
    use super::*;

    #[test]
    fn home_shell_body_leaves_gpu_glass_visible() {
        let mut bytes = vec![0u8; PRIME_LAUNCHER_WIDTH as usize * PRIME_LAUNCHER_HEIGHT as usize * 4];
        let mut canvas = Canvas::new(&mut bytes, PRIME_LAUNCHER_WIDTH, PRIME_LAUNCHER_HEIGHT).expect("home canvas");
        let mut text = TextSystem::load_system().expect("Prime production font");
        paint_prime_launcher_surface(
            &mut canvas,
            &mut text,
            &Theme::prime_dark(),
            PrimeLauncherView {
                applications: &[],
                selected: 0,
                power_ready: true,
                pending_power: None,
                message: None,
                progress: 1.0,
            },
        );
        let sample = canvas.pixel(10, 120).expect("home material sample");
        assert!(sample.a <= 64, "Home hides GPU glass at alpha {}", sample.a);
    }

    #[test]
    fn home_does_not_duplicate_system_power_actions() {
        let layout = PrimeLauncherLayout::new(PRIME_LAUNCHER_WIDTH, PRIME_LAUNCHER_HEIGHT);
        assert_eq!(layout.power_action_at(layout.restart.center_x(), layout.restart.center_y()), None);
        assert_eq!(layout.power_action_at(layout.power_off.center_x(), layout.power_off.center_y()), None);
    }
}
