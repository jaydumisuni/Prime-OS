use super::{draw_icon, Argb, Canvas, FontWeight, Icon, Rect, TextStyle, TextSystem, Theme};

#[cfg(test)]
pub(crate) const PRIMARY_AURORA_BANDS: usize = 10;
#[cfg(test)]
pub(crate) const SECONDARY_AURORA_BANDS: usize = 4;
#[cfg(test)]
pub(crate) const DECORATIVE_BOX_COUNT: usize = 0;
pub(crate) const TOP_STRIP_RULE_Y: i32 = 59;
pub(crate) const STATUS_CLUSTER_WIDTH: u32 = 480;
pub(crate) const STATUS_CLUSTER_HEIGHT: u32 = 44;
pub(crate) const STATUS_CLUSTER_TOP_MARGIN: i32 = 7;
pub(crate) const STATUS_CLUSTER_RIGHT_MARGIN: i32 = 24;

#[cfg(test)]
pub(crate) fn paint_settled_background(canvas: &mut Canvas<'_>, theme: &Theme) {
    paint_background_base(canvas, theme);
    paint_background_motion(canvas, theme, 0.0);
}

pub(crate) fn paint_background_base(canvas: &mut Canvas<'_>, theme: &Theme) {
    canvas.clear();
    if canvas.width == 0 || canvas.height == 0 {
        return;
    }

    let full = Rect::new(0, 0, canvas.width, canvas.height);
    canvas.vertical_gradient(full, theme.base_0, theme.base_1);

    let width = canvas.width as f32;
    let height = canvas.height as f32;
    // Broad Prime light field. All positions and radii are normalized to the
    // output, so there is no bitmap-upscale path at any display resolution.
    canvas.radial_glow(
        width * 0.20,
        height * 0.56,
        width * 0.46,
        theme.violet_alt.with_alpha(104),
    );
    canvas.radial_glow(
        width * 0.43,
        height * 0.69,
        width * 0.32,
        theme.violet.with_alpha(54),
    );
    canvas.radial_glow(
        width * 0.77,
        height * 0.46,
        width * 0.43,
        theme.cyan_alt.with_alpha(96),
    );
    canvas.radial_glow(
        width * 0.61,
        height * 0.67,
        width * 0.28,
        theme.cyan.with_alpha(48),
    );
    canvas.radial_glow(
        width * 0.52,
        height * 0.58,
        width * 0.18,
        theme.text.with_alpha(18),
    );

    canvas.fill_rect(
        Rect::new(0, TOP_STRIP_RULE_Y, canvas.width, 1),
        theme.text.with_alpha(58),
    );
}

fn cubic_point(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
}

fn paint_curve(
    canvas: &mut Canvas<'_>,
    control_x: [f32; 4],
    control_y: [f32; 4],
    y_offset: f32,
    thickness: u32,
    color: Argb,
) {
    let width = canvas.width as f32;
    let height = canvas.height as f32;
    let samples = ((canvas.width / 6).clamp(180, 420)) as usize;
    let mut previous = None;
    for sample in 0..=samples {
        let t = sample as f32 / samples as f32;
        let point = (
            (cubic_point(control_x[0], control_x[1], control_x[2], control_x[3], t) * width).round()
                as i32,
            ((cubic_point(control_y[0], control_y[1], control_y[2], control_y[3], t) + y_offset)
                * height)
                .round() as i32,
        );
        if let Some(last) = previous {
            canvas.line(last, point, thickness, color);
        }
        previous = Some(point);
    }
}

pub(crate) fn paint_background_motion(canvas: &mut Canvas<'_>, theme: &Theme, phase: f32) {
    if canvas.width == 0 || canvas.height == 0 {
        return;
    }

    // Slow counter-phase drift keeps the desktop alive while preserving the
    // approved composition. Motion amplitude is intentionally small so text,
    // windows and the rail remain visually stable.
    let slow = phase.sin();
    let counter = (phase * 0.73 + 1.2).sin();
    let fine = (phase * 1.31 + 0.4).sin();

    let violet_curve_x = [0.0, 0.16, 0.31, 0.58];
    let violet_curve_y = [0.36, 0.43, 0.72, 0.61];
    let violet_drift = slow * 0.010;
    for offset in [-0.018, -0.012, -0.006, 0.0, 0.007, 0.014, 0.020] {
        let center = offset == 0.0;
        paint_curve(
            canvas,
            violet_curve_x,
            violet_curve_y,
            offset + violet_drift,
            1,
            theme.violet.with_alpha(if center {
                104
            } else if offset.abs() <= 0.007 {
                44
            } else {
                20
            }),
        );
    }

    let cyan_curve_x = [0.47, 0.61, 0.77, 1.01];
    let cyan_curve_y = [0.61, 0.59, 0.55, 0.34];
    let cyan_drift = counter * 0.011;
    for offset in [-0.019, -0.012, -0.006, 0.0, 0.007, 0.014, 0.021] {
        let center = offset == 0.0;
        paint_curve(
            canvas,
            cyan_curve_x,
            cyan_curve_y,
            offset + cyan_drift,
            1,
            theme.cyan.with_alpha(if center {
                112
            } else if offset.abs() <= 0.007 {
                46
            } else {
                21
            }),
        );
    }

    let upper_x = [0.61, 0.73, 0.86, 1.02];
    let upper_y = [0.36, 0.43, 0.33, 0.20];
    let upper_drift = fine * 0.008;
    for offset in [-0.010, 0.0, 0.011] {
        paint_curve(
            canvas,
            upper_x,
            upper_y,
            offset + upper_drift,
            1,
            theme.cyan.with_alpha(if offset == 0.0 { 70 } else { 24 }),
        );
    }

    // Sparse moving points provide enough temporal evidence to prove that the
    // idle field is alive without turning the desktop into a particle effect.
    let width = canvas.width as i32;
    let height = canvas.height as i32;
    let drift_x = (slow * canvas.width as f32 * 0.006).round() as i32;
    let drift_y = (counter * canvas.height as f32 * 0.004).round() as i32;
    for index in 0..92_i32 {
        let x = (index * 197 + 83 + drift_x).rem_euclid(width.max(1));
        let y = (index * index * 43 + 61 + drift_y).rem_euclid(height.max(1));
        let reference_zone = (x < width * 2 / 5 && y > height / 8 && y < height * 4 / 5)
            || (x > width / 2 && y < height * 3 / 5);
        if reference_zone && index % 3 != 0 {
            let color = if x < width / 2 {
                theme.violet
            } else {
                theme.cyan
            };
            canvas.circle(x, y, 1, color.with_alpha(36));
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

fn local_clock_label() -> String {
    // SAFETY: libc::time and localtime_r are called with valid pointers to
    // stack-owned values; localtime_r writes exactly one libc::tm.
    unsafe {
        let mut now: libc::time_t = 0;
        if libc::time(&mut now) == -1 {
            return "--:--".to_owned();
        }
        let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
        if libc::localtime_r(&now, local.as_mut_ptr()).is_null() {
            return "--:--".to_owned();
        }
        let local = local.assume_init();
        format!("{:02}:{:02}", local.tm_hour, local.tm_min)
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
        Rect::new(7, 12, 18, 18),
        Icon::Shield,
        theme.text.with_alpha(224),
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

    for x in [178, 282, 382] {
        canvas.fill_rect(Rect::new(x, 8, 1, 27), theme.text.with_alpha(55));
    }
    draw_icon(
        canvas,
        Rect::new(197, 11, 22, 22),
        Icon::Wifi,
        theme.text.with_alpha(224),
    );
    draw_icon(
        canvas,
        Rect::new(241, 11, 22, 22),
        Icon::Audio,
        theme.text.with_alpha(224),
    );
    draw_icon(
        canvas,
        Rect::new(302, 12, 22, 20),
        Icon::Battery,
        theme.text.with_alpha(218),
    );
    text.draw(canvas, (331, 12), "AC", style, theme.muted.with_alpha(228));
    let clock = local_clock_label();
    text.draw(canvas, (409, 12), &clock, style, theme.text.with_alpha(230));
}
