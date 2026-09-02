use std::f32::consts::PI;

use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

pub(crate) const PRIMARY_AURORA_BANDS: usize = 10;
pub(crate) const SECONDARY_AURORA_BANDS: usize = 4;
#[cfg(test)]
pub(crate) const DECORATIVE_BOX_COUNT: usize = 0;
pub(crate) const STATUS_CLUSTER_WIDTH: u32 = 176;
pub(crate) const STATUS_CLUSTER_HEIGHT: u32 = 36;
pub(crate) const STATUS_CLUSTER_TOP_MARGIN: i32 = 8;
pub(crate) const STATUS_CLUSTER_RIGHT_MARGIN: i32 = 12;

pub(crate) fn paint_settled_background(canvas: &mut Canvas<'_>, theme: &Theme) {
    canvas.clear();
    let width = canvas.width;
    let height = canvas.height;
    if width == 0 || height == 0 {
        return;
    }

    canvas.vertical_gradient(Rect::new(0, 0, width, height), theme.base_2, theme.base_0);

    let width_f = width as f32;
    let height_f = height as f32;
    canvas.radial_glow(
        width_f * 0.18,
        height_f * 0.60,
        width_f * 0.52,
        theme.violet.with_alpha(118),
    );
    canvas.radial_glow(
        width_f * 0.38,
        height_f * 0.68,
        width_f * 0.42,
        Argb::from_u32(0xff7c3aed).with_alpha(72),
    );
    canvas.radial_glow(
        width_f * 0.80,
        height_f * 0.46,
        width_f * 0.52,
        theme.cyan.with_alpha(118),
    );
    canvas.radial_glow(
        width_f * 0.54,
        height_f * 0.72,
        width_f * 0.32,
        Argb::from_u32(0x703b82f6),
    );

    paint_aurora_ribbons(canvas, theme);

    let top_rule = theme.text.with_alpha(42);
    canvas.fill_rect(Rect::new(0, 44, width, 1), top_rule);
}

fn paint_aurora_ribbons(canvas: &mut Canvas<'_>, theme: &Theme) {
    let width = canvas.width as i32;
    let height = canvas.height as i32;
    if width <= 1 || height <= 1 {
        return;
    }

    let bands = PRIMARY_AURORA_BANDS;
    for band in 0..bands {
        let t = band as f32 / (bands - 1) as f32;
        let alpha = (4.0 + (1.0 - (t - 0.5).abs() * 2.0) * 8.0) as u8;
        let color = if band < bands / 2 {
            theme.violet.with_alpha(alpha)
        } else {
            theme.cyan.with_alpha(alpha)
        };
        let offset = (band as f32 - bands as f32 / 2.0) * (height as f32 * 0.0065);
        let mut previous: Option<(i32, i32)> = None;
        let step = (width / 180).max(2) as usize;
        for x in (0..width).step_by(step) {
            let progress = x as f32 / width as f32;
            let wave = (progress * PI * 2.15).sin() * height as f32 * 0.085
                + (progress * PI * 0.75).cos() * height as f32 * 0.045;
            let sweep = (0.68 - progress * 0.19) * height as f32;
            let y = (sweep + wave + offset).round() as i32;
            if let Some(last) = previous {
                canvas.line(last, (x, y), if band % 5 == 0 { 2 } else { 1 }, color);
            }
            previous = Some((x, y));
        }
    }

    for band in 0..SECONDARY_AURORA_BANDS {
        let t = band as f32 / (SECONDARY_AURORA_BANDS - 1) as f32;
        let color = Argb::from_u32(0xff60a5fa).with_alpha((4.0 + 8.0 * (1.0 - t)) as u8);
        let offset = band as f32 * height as f32 * 0.007;
        let mut previous = None;
        let step = (width / 150).max(2) as usize;
        for x in (0..width).step_by(step) {
            let progress = x as f32 / width as f32;
            let wave = (progress * PI * 2.8 + 0.6).sin() * height as f32 * 0.035;
            let y = (height as f32 * 0.61 + wave + offset).round() as i32;
            if let Some(last) = previous {
                canvas.line(last, (x, y), 1, color);
            }
            previous = Some((x, y));
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TopStatus {
    Online,
    Limited,
}

impl TopStatus {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Online => "ONLINE",
            Self::Limited => "LIMITED",
        }
    }
}

pub(crate) fn paint_top_status_strip(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    status: TopStatus,
) {
    if canvas.width < 180 || canvas.height < 46 {
        return;
    }
    let brand = TextStyle {
        size_px: 13,
        weight: FontWeight::Semibold,
    };
    let secondary = TextStyle {
        size_px: 12,
        weight: FontWeight::Regular,
    };
    text.draw(
        canvas,
        (18, 12),
        "PRIME OS",
        brand,
        theme.text.with_alpha(236),
    );
    let brand_width = text.measure("PRIME OS", brand).width as i32;
    text.draw(
        canvas,
        (26 + brand_width, 12),
        "First Light",
        secondary,
        theme.muted.with_alpha(230),
    );
    let first_width = text.measure("First Light", secondary).width as i32;
    canvas.circle(
        34 + brand_width + first_width,
        20,
        3,
        theme.cyan.with_alpha(210),
    );

    let _ = status;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StatusClusterLayout {
    pub(crate) bounds: Rect,
}

impl StatusClusterLayout {
    pub(crate) const fn for_surface(width: u32, height: u32) -> Self {
        Self {
            bounds: Rect::new(0, 0, width, height),
        }
    }

    pub(crate) fn hit(self, x: f64, y: f64) -> bool {
        self.bounds.contains(x.floor() as i32, y.floor() as i32)
    }
}

pub(crate) fn paint_status_cluster(
    canvas: &mut Canvas<'_>,
    text: &mut TextSystem,
    theme: &Theme,
    status: TopStatus,
) {
    canvas.clear();
    if canvas.width < 120 || canvas.height < 28 {
        return;
    }
    let body = Rect::new(
        1,
        1,
        canvas.width.saturating_sub(2),
        canvas.height.saturating_sub(2),
    );
    canvas.fill_rounded_rect(body, 17, theme.panel.with_alpha(92));
    canvas.stroke_rounded_rect(body, 17, 1, theme.text.with_alpha(48));
    let style = TextStyle {
        size_px: 11,
        weight: FontWeight::Semibold,
    };
    draw_icon(
        canvas,
        Rect::new(13, 11, 14, 14),
        Icon::Status,
        theme.text.with_alpha(220),
    );
    text.draw(
        canvas,
        (35, 10),
        "STATUS",
        style,
        theme.text.with_alpha(218),
    );
    let label = status.label();
    let label_width = text.measure(label, style).width as i32;
    text.draw(
        canvas,
        (canvas.width as i32 - 14 - label_width, 10),
        label,
        style,
        match status {
            TopStatus::Online => theme.cyan.with_alpha(238),
            TopStatus::Limited => Argb::from_u32(0xfff59e0b).with_alpha(238),
        },
    );
}
