use std::f32::consts::PI;

use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

pub(crate) const PRIMARY_AURORA_BANDS: usize = 10;
pub(crate) const SECONDARY_AURORA_BANDS: usize = 4;

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
        theme.cyan.with_alpha(86),
    );
    canvas.radial_glow(
        width_f * 0.54,
        height_f * 0.72,
        width_f * 0.32,
        Argb::from_u32(0x523b82f6),
    );

    paint_aurora_ribbons(canvas, theme);
    paint_geometric_traces(canvas, theme);

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

fn paint_geometric_traces(canvas: &mut Canvas<'_>, theme: &Theme) {
    let width = canvas.width;
    let height = canvas.height;
    let trace = theme.cyan.with_alpha(22);
    let violet_trace = theme.violet.with_alpha(18);
    let sizes = [
        (width / 3, height / 7, width / 6, height / 8),
        (width / 2, height / 2, width / 7, height / 9),
        (width * 3 / 5, height * 5 / 9, width / 5, height / 7),
    ];
    for (index, (x, y, w, h)) in sizes.into_iter().enumerate() {
        if w < 8 || h < 8 {
            continue;
        }
        canvas.stroke_rounded_rect(
            Rect::new(x as i32, y as i32, w, h),
            18,
            1,
            if index % 2 == 0 { trace } else { violet_trace },
        );
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

    let status_label = status.label();
    let status_width = text.measure(status_label, secondary).width as i32;
    let right_margin = 18;
    let status_x = canvas.width as i32 - right_margin - status_width;
    text.draw(
        canvas,
        (status_x, 12),
        status_label,
        secondary,
        match status {
            TopStatus::Online => theme.cyan.with_alpha(236),
            TopStatus::Limited => Argb::from_u32(0xfff59e0b).with_alpha(236),
        },
    );
    let heading = "STATUS";
    let heading_width = text.measure(heading, secondary).width as i32;
    let icon_size = 14;
    let heading_x = status_x - 18 - heading_width;
    draw_icon(
        canvas,
        Rect::new(
            heading_x - icon_size - 6,
            12,
            icon_size as u32,
            icon_size as u32,
        ),
        Icon::Health,
        theme.text.with_alpha(210),
    );
    text.draw(
        canvas,
        (heading_x, 12),
        heading,
        secondary,
        theme.text.with_alpha(210),
    );
}
