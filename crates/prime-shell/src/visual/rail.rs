use std::{fs, io, path::Path};

use serde_json::{json, Value};

use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

pub(crate) const RAIL_WIDTH: u32 = 132;
pub(crate) const RAIL_LEFT_MARGIN: i32 = 56;
pub(crate) const RAIL_TOP_MARGIN: i32 = 140;
pub(crate) const MAX_RAIL_ITEMS: usize = 8;
const RAIL_VERTICAL_PADDING: u32 = 18;
const RAIL_ITEM_HEIGHT: u32 = 88;
pub(crate) const RAIL_PRIMARY_ICON_SIZE: u32 = 34;
const RAIL_ITEM_GAP: u32 = 8;
const RAIL_SCHEMA: &str = "prime.rail.v1";
pub(crate) const RAIL_GLASS_ALPHA: u8 = 36;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) enum RailAction {
    Prime,
    Status,
    Network,
    Audio,
    Storage,
    Health,
    Application(String),
}

impl RailAction {
    fn icon(&self) -> Icon {
        match self {
            Self::Prime => Icon::Prime,
            Self::Application(_) => Icon::Applications,
            Self::Status => Icon::Status,
            Self::Network => Icon::Network,
            Self::Audio => Icon::Audio,
            Self::Storage => Icon::Storage,
            Self::Health => Icon::Health,
        }
    }

    pub(crate) fn label(&self) -> Option<&'static str> {
        match self {
            Self::Prime => None,
            Self::Status => Some("STATUS"),
            Self::Network => Some("NETWORK"),
            Self::Audio => Some("AUDIO"),
            Self::Storage => Some("STORAGE"),
            Self::Health => Some("HEALTH"),
            Self::Application(_) => Some("APP"),
        }
    }

    fn pin_json(&self) -> Option<Value> {
        match self {
            Self::Prime => None,
            Self::Status => Some(json!({"kind":"status"})),
            Self::Network => Some(json!({"kind":"network"})),
            Self::Audio => Some(json!({"kind":"audio"})),
            Self::Storage => Some(json!({"kind":"storage"})),
            Self::Health => Some(json!({"kind":"health"})),
            Self::Application(application_id) => Some(json!({
                "kind":"application",
                "application_id":application_id,
            })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RailConfiguration {
    actions: Vec<RailAction>,
}

impl Default for RailConfiguration {
    fn default() -> Self {
        Self {
            actions: vec![RailAction::Prime],
        }
    }
}

impl RailConfiguration {
    pub(crate) fn actions(&self) -> &[RailAction] {
        &self.actions
    }

    pub(crate) fn from_json(source: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
        if value.get("schema").and_then(Value::as_str) != Some(RAIL_SCHEMA) {
            return Err(format!("rail schema must be {RAIL_SCHEMA}"));
        }
        let pins = value
            .get("pins")
            .and_then(Value::as_array)
            .ok_or_else(|| "rail pins must be an array".to_owned())?;
        let mut actions = vec![RailAction::Prime];
        for pin in pins {
            if actions.len() >= MAX_RAIL_ITEMS {
                break;
            }
            let kind = pin
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| "rail pin kind must be a string".to_owned())?;
            let action = match kind {
                "prime" | "apps" | "search" => continue,
                "status" => RailAction::Status,
                "network" => RailAction::Network,
                "audio" => RailAction::Audio,
                "storage" => RailAction::Storage,
                "health" => RailAction::Health,
                "application" => {
                    let application_id = pin
                        .get("application_id")
                        .and_then(Value::as_str)
                        .filter(|id| !id.trim().is_empty())
                        .ok_or_else(|| "application rail pin requires application_id".to_owned())?;
                    RailAction::Application(application_id.to_owned())
                }
                other => return Err(format!("unsupported rail pin kind: {other}")),
            };
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
        Ok(Self { actions })
    }

    pub(crate) fn load_from_path(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(source) => match Self::from_json(&source) {
                Ok(config) => {
                    // Persist normalization so legacy Apps/Search pins cannot
                    // reappear on the next login after migration.
                    if config
                        .to_json()
                        .is_ok_and(|normalized| normalized.trim() != source.trim())
                    {
                        let _ = config.save_to_path(path);
                    }
                    config
                }
                Err(_) => Self::default(),
            },
            Err(_) => Self::default(),
        }
    }

    pub(crate) fn save_to_path(&self, path: &Path) -> Result<(), io::Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let encoded = self
            .to_json()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(path, encoded)
    }

    pub(crate) fn to_json(&self) -> Result<String, serde_json::Error> {
        let pins = self
            .actions
            .iter()
            .filter_map(RailAction::pin_json)
            .collect::<Vec<_>>();
        serde_json::to_string_pretty(&json!({
            "schema": RAIL_SCHEMA,
            "pins": pins,
        }))
    }
}

pub(crate) fn rail_height_for_items(item_count: usize) -> u32 {
    let count = item_count.clamp(1, MAX_RAIL_ITEMS) as u32;
    RAIL_VERTICAL_PADDING * 2 + count * RAIL_ITEM_HEIGHT + count.saturating_sub(1) * RAIL_ITEM_GAP
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RailLayout {
    pub(crate) bounds: Rect,
    pub(crate) items: Vec<Rect>,
}

impl RailLayout {
    #[cfg(test)]
    pub(crate) fn for_output(output_width: u32, output_height: u32, item_count: usize) -> Self {
        let desired_height = rail_height_for_items(item_count);
        let max_height = output_height.saturating_sub(RAIL_TOP_MARGIN.max(0) as u32 + 28);
        let height = desired_height.min(max_height.max(RAIL_ITEM_HEIGHT + 20));
        let x = RAIL_LEFT_MARGIN
            .min(output_width.saturating_sub(RAIL_WIDTH) as i32)
            .max(0);
        let y = RAIL_TOP_MARGIN
            .min(output_height.saturating_sub(height) as i32)
            .max(0);
        Self::from_bounds(Rect::new(x, y, RAIL_WIDTH, height), item_count)
    }

    pub(crate) fn for_surface(width: u32, height: u32, item_count: usize) -> Self {
        Self::from_bounds(Rect::new(0, 0, width, height), item_count)
    }

    fn from_bounds(bounds: Rect, item_count: usize) -> Self {
        let count = item_count.clamp(1, MAX_RAIL_ITEMS);
        let item_width = bounds.width.saturating_sub(14).max(40);
        let item_x = bounds.x + ((bounds.width.saturating_sub(item_width)) / 2) as i32;
        let available = bounds.height.saturating_sub(RAIL_VERTICAL_PADDING * 2);
        let gap_total = RAIL_ITEM_GAP * count.saturating_sub(1) as u32;
        let item_height = RAIL_ITEM_HEIGHT.min(available.saturating_sub(gap_total) / count as u32);
        let items = (0..count)
            .map(|index| {
                Rect::new(
                    item_x,
                    bounds.y
                        + RAIL_VERTICAL_PADDING as i32
                        + index as i32 * (item_height + RAIL_ITEM_GAP) as i32,
                    item_width,
                    item_height,
                )
            })
            .collect();
        Self { bounds, items }
    }

    pub(crate) fn hit(&self, x: f64, y: f64, actions: &[RailAction]) -> Option<RailAction> {
        let x = x.floor() as i32;
        let y = y.floor() as i32;
        self.items
            .iter()
            .zip(actions.iter())
            .find_map(|(rect, action)| rect.contains(x, y).then(|| action.clone()))
    }
}

const PRIME_LOGO_48: &[u8] = include_bytes!("../../assets/branding/prime-logo-48.rgba");
const PRIME_LOGO_64: &[u8] = include_bytes!("../../assets/branding/prime-logo-64.rgba");
const PRIME_LOGO_80: &[u8] = include_bytes!("../../assets/branding/prime-logo-80.rgba");
const PRIME_LOGO_128: &[u8] = include_bytes!("../../assets/branding/prime-logo-128.rgba");

fn prime_logo_asset(target_size: u32) -> (&'static [u8], u32) {
    if target_size <= 48 {
        (PRIME_LOGO_48, 48)
    } else if target_size <= 64 {
        (PRIME_LOGO_64, 64)
    } else if target_size <= 80 {
        (PRIME_LOGO_80, 80)
    } else {
        (PRIME_LOGO_128, 128)
    }
}

fn logo_pixel(source: &[u8], source_size: u32, x: u32, y: u32) -> [f32; 4] {
    let offset = ((y * source_size + x) * 4) as usize;
    let alpha = f32::from(source[offset + 3]) / 255.0;
    [
        f32::from(source[offset]) * alpha,
        f32::from(source[offset + 1]) * alpha,
        f32::from(source[offset + 2]) * alpha,
        f32::from(source[offset + 3]),
    ]
}

fn sample_logo_bilinear(source: &[u8], source_size: u32, x: f32, y: f32) -> Argb {
    let max = source_size.saturating_sub(1);
    let x = x.clamp(0.0, max as f32);
    let y = y.clamp(0.0, max as f32);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(max);
    let y1 = (y0 + 1).min(max);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let p00 = logo_pixel(source, source_size, x0, y0);
    let p10 = logo_pixel(source, source_size, x1, y0);
    let p01 = logo_pixel(source, source_size, x0, y1);
    let p11 = logo_pixel(source, source_size, x1, y1);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let mut mixed = [0.0_f32; 4];
    for channel in 0..4 {
        let top = lerp(p00[channel], p10[channel], tx);
        let bottom = lerp(p01[channel], p11[channel], tx);
        mixed[channel] = lerp(top, bottom, ty);
    }
    let alpha = mixed[3].round().clamp(0.0, 255.0) as u8;
    if alpha == 0 {
        return Argb {
            a: 0,
            r: 0,
            g: 0,
            b: 0,
        };
    }
    let alpha_scale = 255.0 / f32::from(alpha);
    Argb {
        a: alpha,
        r: (mixed[0] * alpha_scale).round().clamp(0.0, 255.0) as u8,
        g: (mixed[1] * alpha_scale).round().clamp(0.0, 255.0) as u8,
        b: (mixed[2] * alpha_scale).round().clamp(0.0, 255.0) as u8,
    }
}

pub(crate) fn draw_prime_mark(canvas: &mut Canvas<'_>, rect: Rect, theme: &Theme, active: bool) {
    let size = rect.width.min(rect.height).clamp(1, 220);
    let (source, source_size) = prime_logo_asset(size);
    debug_assert_eq!(source.len(), (source_size * source_size * 4) as usize);
    let center_x = rect.center_x() as f32;
    let center_y = rect.center_y() as f32;
    let x0 = rect.center_x().round() as i32 - size as i32 / 2;
    let y0 = rect.center_y().round() as i32 - size as i32 / 2;

    // The approved Prime mark always carries a restrained bloom. Active state
    // increases the same bloom without sacrificing the sharp vector-like edge.
    canvas.radial_glow(
        center_x,
        center_y,
        size as f32 * 0.78,
        theme.violet.with_alpha(if active { 118 } else { 58 }),
    );
    canvas.radial_glow(
        center_x + size as f32 * 0.12,
        center_y,
        size as f32 * 0.62,
        theme.cyan.with_alpha(if active { 82 } else { 42 }),
    );

    let scale = source_size as f32 / size as f32;
    for y in 0..size {
        for x in 0..size {
            let source_x = (x as f32 + 0.5) * scale - 0.5;
            let source_y = (y as f32 + 0.5) * scale - 0.5;
            let mut color = sample_logo_bilinear(source, source_size, source_x, source_y);
            if !active {
                color.a = ((u16::from(color.a) * 248) / 255) as u8;
            }
            if color.a != 0 {
                canvas.blend_pixel(x0 + x as i32, y0 + y as i32, color);
            }
        }
    }
}

pub(crate) fn paint_rail_surface(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    actions: &[RailAction],
    active: Option<RailAction>,
) {
    canvas.clear();
    let layout = RailLayout::for_surface(canvas.width, canvas.height, actions.len());
    let body = Rect::new(
        1,
        1,
        canvas.width.saturating_sub(2),
        canvas.height.saturating_sub(2),
    );
    canvas.fill_rounded_rect(body, 24, theme.panel.with_alpha(RAIL_GLASS_ALPHA));
    canvas.stroke_rounded_rect(body, 24, 1, theme.cyan.with_alpha(74));

    for (rect, action) in layout.items.iter().zip(actions.iter()) {
        let is_active = active.as_ref() == Some(action);
        if is_active {
            canvas.fill_rounded_rect(*rect, 18, theme.violet.with_alpha(24));
            canvas.stroke_rounded_rect(*rect, 18, 1, theme.cyan.with_alpha(112));
        }
        if matches!(action, RailAction::Prime) {
            let mark = Rect::new(rect.x + 29, rect.y + 16, rect.width.saturating_sub(58), 48);
            draw_prime_mark(canvas, mark, theme, is_active);
            let label = "PRIME OS";
            let style = TextStyle {
                size_px: 11,
                weight: FontWeight::Semibold,
            };
            let label_width = text.measure(label, style).width as i32;
            text.draw(
                canvas,
                (rect.center_x() as i32 - label_width / 2, rect.y + 64),
                label,
                style,
                theme.text.with_alpha(234),
            );
            continue;
        }

        let icon_size = RAIL_PRIMARY_ICON_SIZE;
        let icon_rect = Rect::new(
            rect.center_x().round() as i32 - icon_size as i32 / 2,
            rect.y + 17,
            icon_size,
            icon_size,
        );
        let color = if is_active {
            theme.cyan
        } else {
            theme.text.with_alpha(226)
        };
        draw_icon(canvas, icon_rect, action.icon(), color);
        if let Some(label) = action.label() {
            let style = TextStyle {
                size_px: 11,
                weight: FontWeight::Regular,
            };
            let width = text.measure(label, style).width as i32;
            text.draw(
                canvas,
                (rect.center_x().round() as i32 - width / 2, rect.y + 55),
                label,
                style,
                theme.text.with_alpha(205),
            );
        }
    }
}

/// Full-height preview taskbar. Native Prime rail remains independently available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewRailHit {
    Prime,
    Running(usize),
    PreviousWindows,
    NextWindows,
    Pinned(usize),
    QuickControls,
    Outside,
}

pub(crate) const PREVIEW_RAIL_WIDTH: u32 = 69;

pub(crate) fn preview_rail_hit(
    x: f64,
    y: f64,
    width: u32,
    height: u32,
    windows: usize,
    offset: usize,
    pins: usize,
) -> PreviewRailHit {
    let (x, y) = (x.floor() as i32, y.floor() as i32);
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return PreviewRailHit::Outside;
    }
    if y < 103 {
        return PreviewRailHit::Prime;
    }
    if (125..=390).contains(&y) {
        if y >= 145 {
            let index = (y - 145) / 48;
            if (0..5).contains(&index) && offset + (index as usize) < windows {
                return PreviewRailHit::Running(offset + index as usize);
            }
        }
    }
    if (392..421).contains(&y) && windows > 5 {
        return if x < width as i32 / 2 {
            PreviewRailHit::PreviousWindows
        } else {
            PreviewRailHit::NextWindows
        };
    }
    let pinned_start = if height > 740 {
        440
    } else {
        height.saturating_sub(271) as i32
    };
    if y >= pinned_start + 25 && y < pinned_start + 25 + ((pins.min(3)) as i32 * 44) {
        return PreviewRailHit::Pinned(((y - pinned_start - 25) / 44) as usize);
    }
    if y >= height as i32 - 139 {
        return PreviewRailHit::QuickControls;
    }
    PreviewRailHit::Outside
}

fn window_icon<'a>(
    window: &crate::core_client::PreviewWindow,
    entries: &'a [prime_contracts::ApplicationEntry],
    icons: &'a std::collections::HashMap<String, super::wallpaper::DecodedWallpaper>,
) -> Option<&'a super::wallpaper::DecodedWallpaper> {
    let id = window.app_id.to_lowercase();
    let name = if id.contains("thunar") {
        "files"
    } else if id.contains("settings") {
        "settings"
    } else if id.contains("ptyxis") || id.contains("terminal") {
        "terminal"
    } else if id.contains("brave") {
        "browser"
    } else if id.contains("firefox") {
        "firefox"
    } else if id.contains("chrome") {
        "chrome"
    } else {
        id.as_str()
    };
    entries
        .iter()
        .find(|entry| entry.display_name.to_lowercase().contains(name))
        .and_then(|entry| icons.get(&entry.application_id.to_string()))
}

pub(crate) fn paint_preview_rail_surface(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    running: &[crate::core_client::PreviewWindow],
    apps: &[prime_contracts::ApplicationEntry],
    icons: &std::collections::HashMap<String, super::wallpaper::DecodedWallpaper>,
    pins: &[String],
    running_offset: usize,
) {
    canvas.clear();
    let h = canvas.height as i32;
    let width = canvas.width;
    let center = width as i32 / 2;
    let body = Rect::new(
        1,
        1,
        width.saturating_sub(2),
        canvas.height.saturating_sub(2),
    );
    canvas.fill_rounded_rect(body, 25, theme.base_0.with_alpha(220));
    canvas.stroke_rounded_rect(body, 25, 1, theme.cyan.with_alpha(157));
    let logo = Rect::new(center - 27, 7, 54, 53);
    draw_prime_mark(canvas, logo, theme, true);
    let label = "Prime OS";
    let style = TextStyle {
        size_px: 11,
        weight: FontWeight::Semibold,
    };
    text.draw(
        canvas,
        (center - text.measure(label, style).width as i32 / 2, 66),
        label,
        style,
        theme.text,
    );
    let heading = TextStyle {
        size_px: 10,
        weight: FontWeight::Semibold,
    };
    text.draw(canvas, (15, 118), "Running", heading, theme.text);

    for (index, window) in running.iter().skip(running_offset).take(5).enumerate() {
        let y = 145 + index as i32 * 48;
        let tile = Rect::new(center - 25, y, 50, 45);
        canvas.fill_rounded_rect(tile, 11, theme.panel.with_alpha(70));
        let custom = {
            let app_id = window.app_id.to_lowercase();
            let alias = if app_id.contains("thunar") {
                "Files"
            } else if app_id.contains("settings") {
                "Settings"
            } else if app_id.contains("ptyxis") || app_id.contains("terminal") {
                "Terminal"
            } else {
                ""
            };
            super::reference_icons::system_icon_for(alias)
        };
        if let Some(icon) = custom.or_else(|| window_icon(window, apps, icons)) {
            super::wallpaper::paint_icon_image(
                canvas,
                icon,
                Rect::new(center - 20, y + 4, 40, 37),
                255,
            );
        } else {
            draw_icon(
                canvas,
                Rect::new(center - 17, y + 6, 34, 34),
                Icon::Applications,
                theme.muted,
            );
        }
        canvas.fill_rounded_rect(Rect::new(9, y + 11, 3, 23), 2, theme.cyan.with_alpha(215));
    }
    if running.len() > 5 {
        let y = 397;
        text.draw(
            canvas,
            (18, y),
            "‹",
            TextStyle {
                size_px: 18,
                weight: FontWeight::Semibold,
            },
            theme.cyan,
        );
        text.draw(
            canvas,
            (center + 13, y),
            "›",
            TextStyle {
                size_px: 18,
                weight: FontWeight::Semibold,
            },
            theme.cyan,
        );
    }
    let py = if h > 740 { 440 } else { h.saturating_sub(271) };
    canvas.fill_rect(
        Rect::new(15, py - 14, width.saturating_sub(30), 1),
        theme.text.with_alpha(52),
    );
    text.draw(canvas, (16, py), "Pinned", heading, theme.text);
    for (index, id) in pins.iter().take(3).enumerate() {
        let y = py + 25 + index as i32 * 44;
        let app = apps
            .iter()
            .find(|item| item.application_id.to_string() == *id);
        let art = app
            .and_then(|item| super::reference_icons::system_icon_for(&item.display_name))
            .or_else(|| app.and_then(|_item| icons.get(id)));
        let rect = Rect::new(center - 21, y + 2, 42, 42);
        if let Some(image) = art {
            super::wallpaper::paint_icon_image(canvas, image, rect, 255);
        } else {
            draw_icon(canvas, rect, Icon::Applications, theme.muted);
        }
    }
    let quick = h - 139;
    canvas.fill_rect(
        Rect::new(14, quick - 9, width.saturating_sub(28), 1),
        theme.text.with_alpha(44),
    );
    for (row, (name, fallback)) in [
        ("Wi-Fi", Icon::Network),
        ("Bluetooth", Icon::Audio),
        ("Performance", Icon::Settings),
    ]
    .iter()
    .enumerate()
    {
        let rect = Rect::new(center - 16, quick + row as i32 * 34, 32, 32);
        if let Some(image) = super::reference_icons::quick_icon_for(name) {
            super::reference_icons::paint_icon(canvas, image, rect, 255);
        } else {
            draw_icon(canvas, rect, *fallback, theme.cyan);
        }
    }
    let clock = "Quick";
    text.draw(
        canvas,
        (
            center - text.measure(clock, heading).width as i32 / 2,
            h - 30,
        ),
        clock,
        heading,
        theme.muted,
    );
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    #[test]
    fn five_window_slots_are_bounded_and_both_overflow_halves_reachable() {
        let w = PREVIEW_RAIL_WIDTH;
        assert_eq!(
            preview_rail_hit(40.0, 24.0, w, 718, 9, 0, 3),
            PreviewRailHit::Prime
        );
        for i in 0..5 {
            assert_eq!(
                preview_rail_hit(40.0, 150.0 + i as f64 * 48.0, w, 718, 9, 0, 3),
                PreviewRailHit::Running(i)
            );
        }
        assert_eq!(
            preview_rail_hit(12.0, 405.0, w, 718, 9, 0, 3),
            PreviewRailHit::PreviousWindows
        );
        assert_eq!(
            preview_rail_hit(60.0, 405.0, w, 718, 9, 0, 3),
            PreviewRailHit::NextWindows
        );
        assert_eq!(
            preview_rail_hit(60.0, 405.0, w, 718, 5, 0, 3),
            PreviewRailHit::Outside
        );
        assert_eq!(
            preview_rail_hit(41.0, 683.0, w, 718, 9, 0, 3),
            PreviewRailHit::QuickControls
        );
    }
}
