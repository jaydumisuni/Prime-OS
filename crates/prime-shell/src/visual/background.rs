use std::f32::consts::PI;

use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

pub(crate) const PRIMARY_AURORA_BANDS: usize = 10;
pub(crate) const SECONDARY_AURORA_BANDS: usize = 4;
#[cfg(test)]
pub(crate) const DECORATIVE_BOX_COUNT: usize = 0;
pub(crate) const TOP_STRIP_RULE_Y: i32 = 59;
pub(crate) const STATUS_CLUSTER_WIDTH: u32 = 480;
pub(crate) const STATUS_CLUSTER_HEIGHT: u32 = 44;
pub(crate) const STATUS_CLUSTER_TOP_MARGIN: i32 = 7;
pub(crate) const STATUS_CLUSTER_RIGHT_MARGIN: i32 = 24;

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
    canvas.fill_rect(Rect::new(0, TOP_STRIP_RULE_Y, width, 1), top_rule);
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
            Self::Online => "NOMINAL",
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
    if canvas.width < 420 || canvas.height < 120 {
        return;
    }
    let brand = TextStyle {
        size_px: 14,
        weight: FontWeight::Semibold,
    };
    let secondary = TextStyle {
        size_px: 12,
        weight: FontWeight::Regular,
    };
    text.draw(
        canvas,
        (36, 17),
        "PRIME OS",
        brand,
        theme.text.with_alpha(242),
    );
    let brand_width = text.measure("PRIME OS", brand).width as i32;
    text.draw(
        canvas,
        (48 + brand_width, 18),
        "First Light",
        secondary,
        theme.muted.with_alpha(232),
    );
    let first_width = text.measure("First Light", secondary).width as i32;
    canvas.circle(
        58 + brand_width + first_width,
        27,
        4,
        theme.text.with_alpha(70),
    );
    canvas.circle(
        58 + brand_width + first_width,
        27,
        2,
        theme.cyan.with_alpha(220),
    );

    if canvas.height >= 1040 {
        let identity_y = canvas.height as i32 - 102;
        let build_y = canvas.height as i32 - 77;
        text.draw(
            canvas,
            (68, identity_y),
            "PRIME OS",
            TextStyle {
                size_px: 18,
                weight: FontWeight::Semibold,
            },
            theme.text.with_alpha(232),
        );
        let idw = text
            .measure(
                "PRIME OS",
                TextStyle {
                    size_px: 18,
                    weight: FontWeight::Semibold,
                },
            )
            .width as i32;
        text.draw(
            canvas,
            (80 + idw, identity_y + 1),
            "First Light",
            TextStyle {
                size_px: 16,
                weight: FontWeight::Regular,
            },
            Argb::from_u32(0xff60a5fa).with_alpha(230),
        );
        text.draw(
            canvas,
            (68, build_y),
            "KRATOS // BUILD 0.1.0",
            TextStyle {
                size_px: 10,
                weight: FontWeight::Regular,
            },
            theme.muted.with_alpha(190),
        );

        let watermark_y = canvas.height as i32 - 92;
        let watermark_x = canvas.width as i32 - 145;
        canvas.circle(watermark_x, watermark_y + 7, 12, theme.text.with_alpha(20));
        canvas.circle(watermark_x, watermark_y + 7, 4, theme.cyan.with_alpha(38));
        text.draw(
            canvas,
            (watermark_x + 27, watermark_y),
            "PRIME",
            TextStyle {
                size_px: 17,
                weight: FontWeight::Regular,
            },
            theme.text.with_alpha(34),
        );
    }
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
    if canvas.width < 360 || canvas.height < 36 {
        return;
    }
    let style = TextStyle {
        size_px: 11,
        weight: FontWeight::Semibold,
    };
    draw_icon(
        canvas,
        Rect::new(8, 14, 14, 14),
        Icon::Status,
        theme.text.with_alpha(220),
    );
    text.draw(
        canvas,
        (29, 12),
        "STATUS",
        style,
        theme.text.with_alpha(222),
    );
    canvas.circle(
        88,
        21,
        4,
        match status {
            TopStatus::Online => Argb::from_u32(0xff34d399),
            TopStatus::Limited => Argb::from_u32(0xfff59e0b),
        },
    );
    text.draw(
        canvas,
        (100, 12),
        status.label(),
        style,
        theme.muted.with_alpha(230),
    );

    for x in [184, 248, 312, 376] {
        canvas.fill_rect(Rect::new(x, 8, 1, 27), theme.text.with_alpha(55));
    }
    draw_icon(
        canvas,
        Rect::new(204, 13, 18, 18),
        Icon::Network,
        theme.text.with_alpha(220),
    );
    draw_icon(
        canvas,
        Rect::new(268, 13, 18, 18),
        Icon::Audio,
        theme.text.with_alpha(220),
    );
    draw_icon(
        canvas,
        Rect::new(332, 13, 18, 18),
        Icon::Power,
        theme.text.with_alpha(210),
    );
    text.draw(
        canvas,
        (397, 12),
        "PRIME",
        style,
        theme.text.with_alpha(185),
    );
}
