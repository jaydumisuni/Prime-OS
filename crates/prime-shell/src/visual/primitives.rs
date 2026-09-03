use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Argb {
    pub(crate) a: u8,
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

impl Argb {
    pub(crate) const TRANSPARENT: Self = Self {
        a: 0,
        r: 0,
        g: 0,
        b: 0,
    };

    pub(crate) const fn from_u32(value: u32) -> Self {
        Self {
            a: ((value >> 24) & 0xff) as u8,
            r: ((value >> 16) & 0xff) as u8,
            g: ((value >> 8) & 0xff) as u8,
            b: (value & 0xff) as u8,
        }
    }

    pub(crate) fn over(self, destination: Self) -> Self {
        let source_alpha = u32::from(self.a);
        let destination_alpha = u32::from(destination.a);
        let inverse_source = 255 - source_alpha;
        let output_alpha = source_alpha + (destination_alpha * inverse_source + 127) / 255;
        if output_alpha == 0 {
            return Self::TRANSPARENT;
        }

        Self {
            a: output_alpha.min(255) as u8,
            r: composite_channel(
                self.r,
                destination.r,
                source_alpha,
                destination_alpha,
                inverse_source,
                output_alpha,
            ),
            g: composite_channel(
                self.g,
                destination.g,
                source_alpha,
                destination_alpha,
                inverse_source,
                output_alpha,
            ),
            b: composite_channel(
                self.b,
                destination.b,
                source_alpha,
                destination_alpha,
                inverse_source,
                output_alpha,
            ),
        }
    }

    pub(crate) fn with_alpha(self, alpha: u8) -> Self {
        Self { a: alpha, ..self }
    }

    fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            a: lerp_channel(self.a, other.a, t),
            r: lerp_channel(self.r, other.r, t),
            g: lerp_channel(self.g, other.g, t),
            b: lerp_channel(self.b, other.b, t),
        }
    }

    fn to_premultiplied_u32(self) -> u32 {
        let alpha = u32::from(self.a);
        let premultiply = |channel: u8| (u32::from(channel) * alpha + 127) / 255;
        (alpha << 24)
            | (premultiply(self.r) << 16)
            | (premultiply(self.g) << 8)
            | premultiply(self.b)
    }

    fn from_premultiplied_u32(value: u32) -> Self {
        let alpha = ((value >> 24) & 0xff) as u8;
        if alpha == 0 {
            return Self::TRANSPARENT;
        }
        let unpremultiply = |channel: u8| {
            ((u32::from(channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
        };
        Self {
            a: alpha,
            r: unpremultiply(((value >> 16) & 0xff) as u8),
            g: unpremultiply(((value >> 8) & 0xff) as u8),
            b: unpremultiply((value & 0xff) as u8),
        }
    }
}

fn composite_channel(
    source: u8,
    destination: u8,
    source_alpha: u32,
    destination_alpha: u32,
    inverse_source: u32,
    output_alpha: u32,
) -> u8 {
    let source_premultiplied = u32::from(source) * source_alpha;
    let destination_premultiplied =
        (u32::from(destination) * destination_alpha * inverse_source + 127) / 255;
    ((source_premultiplied + destination_premultiplied + output_alpha / 2) / output_alpha).min(255)
        as u8
}

fn lerp_channel(start: u8, end: u8, t: f32) -> u8 {
    (f32::from(start) + (f32::from(end) - f32::from(start)) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Rect {
    pub(crate) const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub(crate) fn contains(self, x: i32, y: i32) -> bool {
        let right = i64::from(self.x) + i64::from(self.width);
        let bottom = i64::from(self.y) + i64::from(self.height);
        i64::from(x) >= i64::from(self.x)
            && i64::from(y) >= i64::from(self.y)
            && i64::from(x) < right
            && i64::from(y) < bottom
    }

    pub(crate) fn center_x(self) -> f64 {
        f64::from(self.x) + f64::from(self.width) / 2.0
    }

    pub(crate) fn center_y(self) -> f64 {
        f64::from(self.y) + f64::from(self.height) / 2.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CanvasError {
    SizeOverflow,
    BufferTooSmall,
}

impl fmt::Display for CanvasError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SizeOverflow => formatter.write_str("Prime canvas dimensions overflow"),
            Self::BufferTooSmall => {
                formatter.write_str("Prime canvas buffer is smaller than its declared dimensions")
            }
        }
    }
}

impl Error for CanvasError {}

pub(crate) struct Canvas<'a> {
    bytes: &'a mut [u8],
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl<'a> Canvas<'a> {
    pub(crate) fn new(bytes: &'a mut [u8], width: u32, height: u32) -> Result<Self, CanvasError> {
        let required = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(CanvasError::SizeOverflow)?;
        if bytes.len() < required {
            return Err(CanvasError::BufferTooSmall);
        }
        Ok(Self {
            bytes,
            width,
            height,
        })
    }

    pub(crate) fn clear(&mut self) {
        let required = self.width as usize * self.height as usize * 4;
        self.bytes[..required].fill(0);
    }

    pub(crate) fn pixel(&self, x: i32, y: i32) -> Option<Argb> {
        let offset = self.offset(x, y)?;
        let value = u32::from_le_bytes(self.bytes[offset..offset + 4].try_into().ok()?);
        Some(Argb::from_premultiplied_u32(value))
    }

    pub(crate) fn blend_pixel(&mut self, x: i32, y: i32, color: Argb) {
        let Some(destination) = self.pixel(x, y) else {
            return;
        };
        self.replace_pixel(x, y, color.over(destination));
    }

    pub(crate) fn fill_rect(&mut self, rect: Rect, color: Argb) {
        let (start_x, start_y, end_x, end_y) = self.clamped_bounds(rect);
        for y in start_y..end_y {
            for x in start_x..end_x {
                self.blend_pixel(x, y, color);
            }
        }
    }

    pub(crate) fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Argb) {
        self.paint_rounded_region(rect, radius, None, color);
    }

    pub(crate) fn stroke_rounded_rect(
        &mut self,
        rect: Rect,
        radius: u32,
        thickness: u32,
        color: Argb,
    ) {
        if thickness == 0 || rect.width == 0 || rect.height == 0 {
            return;
        }
        let inset = thickness.min(rect.width / 2).min(rect.height / 2);
        let inner = (rect.width > inset * 2 && rect.height > inset * 2).then(|| {
            Rect::new(
                rect.x.saturating_add(inset as i32),
                rect.y.saturating_add(inset as i32),
                rect.width - inset * 2,
                rect.height - inset * 2,
            )
        });
        self.paint_rounded_region(
            rect,
            radius,
            inner.map(|r| (r, radius.saturating_sub(inset))),
            color,
        );
    }

    pub(crate) fn vertical_gradient(&mut self, rect: Rect, top: Argb, bottom: Argb) {
        if rect.height == 0 {
            return;
        }
        for row in 0..rect.height {
            let t = if rect.height == 1 {
                0.0
            } else {
                row as f32 / (rect.height - 1) as f32
            };
            self.fill_rect(
                Rect::new(rect.x, rect.y.saturating_add(row as i32), rect.width, 1),
                top.mix(bottom, t),
            );
        }
    }

    pub(crate) fn radial_glow(&mut self, center_x: f32, center_y: f32, radius: f32, color: Argb) {
        if radius <= 0.0 {
            return;
        }
        let min_x = (center_x - radius).floor() as i32;
        let max_x = (center_x + radius).ceil() as i32;
        let min_y = (center_y - radius).floor() as i32;
        let max_y = (center_y + radius).ceil() as i32;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 + 0.5 - center_x;
                let dy = y as f32 + 0.5 - center_y;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance <= radius {
                    let falloff = 1.0 - distance / radius;
                    let alpha = (f32::from(color.a) * falloff * falloff).round() as u8;
                    self.blend_pixel(x, y, color.with_alpha(alpha));
                }
            }
        }
    }

    pub(crate) fn circle(&mut self, center_x: i32, center_y: i32, radius: u32, color: Argb) {
        let radius = radius as i32;
        let radius_squared = i64::from(radius) * i64::from(radius);
        for y in center_y.saturating_sub(radius)..=center_y.saturating_add(radius) {
            for x in center_x.saturating_sub(radius)..=center_x.saturating_add(radius) {
                let dx = i64::from(x - center_x);
                let dy = i64::from(y - center_y);
                if dx * dx + dy * dy <= radius_squared {
                    self.blend_pixel(x, y, color);
                }
            }
        }
    }

    pub(crate) fn line(&mut self, start: (i32, i32), end: (i32, i32), thickness: u32, color: Argb) {
        let (mut x0, mut y0) = start;
        let (x1, y1) = end;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        let radius = thickness.saturating_sub(1) / 2;

        loop {
            if radius == 0 {
                self.blend_pixel(x0, y0, color);
            } else {
                self.circle(x0, y0, radius, color);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let doubled = error * 2;
            if doubled >= dy {
                error += dy;
                x0 += sx;
            }
            if doubled <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }

    fn paint_rounded_region(
        &mut self,
        outer: Rect,
        outer_radius: u32,
        excluded_inner: Option<(Rect, u32)>,
        color: Argb,
    ) {
        let radius = outer_radius.min(outer.width / 2).min(outer.height / 2) as i32;
        let (start_x, start_y, end_x, end_y) = self.clamped_bounds(outer);
        for y in start_y..end_y {
            for x in start_x..end_x {
                if !rounded_contains(outer, radius, x, y) {
                    continue;
                }
                if excluded_inner.is_some_and(|(inner, inner_radius)| {
                    rounded_contains(inner, inner_radius as i32, x, y)
                }) {
                    continue;
                }
                self.blend_pixel(x, y, color);
            }
        }
    }

    fn replace_pixel(&mut self, x: i32, y: i32, color: Argb) {
        let Some(offset) = self.offset(x, y) else {
            return;
        };
        self.bytes[offset..offset + 4].copy_from_slice(&color.to_premultiplied_u32().to_le_bytes());
    }

    fn offset(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        (y as usize)
            .checked_mul(self.width as usize)
            .and_then(|row| row.checked_add(x as usize))
            .and_then(|pixel| pixel.checked_mul(4))
    }

    fn clamped_bounds(&self, rect: Rect) -> (i32, i32, i32, i32) {
        let start_x = rect.x.max(0).min(self.width as i32);
        let start_y = rect.y.max(0).min(self.height as i32);
        let end_x = rect
            .x
            .saturating_add(rect.width as i32)
            .max(0)
            .min(self.width as i32);
        let end_y = rect
            .y
            .saturating_add(rect.height as i32)
            .max(0)
            .min(self.height as i32);
        (start_x, start_y, end_x, end_y)
    }
}

fn rounded_contains(rect: Rect, radius: i32, x: i32, y: i32) -> bool {
    if radius <= 0 {
        return rect.contains(x, y);
    }
    let radius = radius
        .min((rect.width / 2) as i32)
        .min((rect.height / 2) as i32);
    let left = rect.x;
    let top = rect.y;
    let right = rect.x.saturating_add(rect.width as i32);
    let bottom = rect.y.saturating_add(rect.height as i32);
    if x < left || y < top || x >= right || y >= bottom {
        return false;
    }

    let inner_left = left + radius;
    let inner_right = right - radius;
    let inner_top = top + radius;
    let inner_bottom = bottom - radius;
    if x >= inner_left && x < inner_right || y >= inner_top && y < inner_bottom {
        return true;
    }

    let center_x = if x < inner_left {
        inner_left
    } else {
        inner_right - 1
    };
    let center_y = if y < inner_top {
        inner_top
    } else {
        inner_bottom - 1
    };
    let dx = i64::from(x - center_x);
    let dy = i64::from(y - center_y);
    dx * dx + dy * dy <= i64::from(radius) * i64::from(radius)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Icon {
    Prime,
    Applications,
    Files,
    Terminal,
    Browser,
    Settings,
    Media,
    Recovery,
    Status,
    Shield,
    Wifi,
    Battery,
    Network,
    Audio,
    Storage,
    Health,
    Restart,
    Power,
    Search,
    Chevron,
}

fn draw_arc(
    canvas: &mut Canvas<'_>,
    center: (i32, i32),
    radius: f32,
    angles: (f32, f32),
    steps: u32,
    thickness: u32,
    color: Argb,
) {
    let (center_x, center_y) = center;
    let (start_radians, end_radians) = angles;
    let mut previous = None;
    for index in 0..=steps.max(4) {
        let t = index as f32 / steps.max(4) as f32;
        let angle = start_radians + (end_radians - start_radians) * t;
        let point = (
            (center_x as f32 + radius * angle.cos()).round() as i32,
            (center_y as f32 + radius * angle.sin()).round() as i32,
        );
        if let Some(last) = previous {
            canvas.line(last, point, thickness.max(1), color);
        }
        previous = Some(point);
    }
}

pub(crate) fn draw_icon(canvas: &mut Canvas<'_>, rect: Rect, icon: Icon, color: Argb) {
    let center_x = rect.x.saturating_add((rect.width / 2) as i32);
    let center_y = rect.y.saturating_add((rect.height / 2) as i32);
    let scale = rect.width.min(rect.height).max(8);
    let stroke = (scale / 10).max(1);
    let radius = (scale / 3).max(2);

    match icon {
        Icon::Prime => {
            canvas.circle(center_x, center_y, radius, color.with_alpha(58));
            canvas.circle(center_x, center_y, (scale / 8).max(2), color);
            canvas.line(
                (center_x - radius as i32, center_y),
                (center_x + radius as i32, center_y),
                stroke,
                color.with_alpha(180),
            );
        }
        Icon::Applications => {
            let cell = (scale * 7 / 25).max(5);
            let gap = (scale / 6).max(3);
            let total = cell * 2 + gap;
            let start_x = center_x - (total / 2) as i32;
            let start_y = center_y - (total / 2) as i32;
            for row in 0..2 {
                for column in 0..2 {
                    canvas.stroke_rounded_rect(
                        Rect::new(
                            start_x + (column * (cell + gap)) as i32,
                            start_y + (row * (cell + gap)) as i32,
                            cell,
                            cell,
                        ),
                        (cell / 3).max(2),
                        stroke.max(2),
                        color,
                    );
                }
            }
        }
        Icon::Files => {
            let r = radius as i32;
            let body = Rect::new(
                center_x - r,
                center_y - r / 2,
                radius * 2,
                radius + radius / 2,
            );
            let tab = Rect::new(
                center_x - r + 2,
                center_y - r + 1,
                radius,
                (radius / 2).max(4),
            );
            canvas.fill_rounded_rect(tab, (scale / 12).max(2), color.with_alpha(38));
            canvas.stroke_rounded_rect(
                tab,
                (scale / 12).max(2),
                stroke.max(1),
                color.with_alpha(215),
            );
            canvas.fill_rounded_rect(body, (scale / 9).max(3), color.with_alpha(24));
            canvas.stroke_rounded_rect(body, (scale / 9).max(3), stroke.max(2), color);
            canvas.line(
                (body.x + 4, body.y + 5),
                (body.x + body.width as i32 - 4, body.y + 5),
                stroke.max(1),
                color.with_alpha(150),
            );
        }
        Icon::Terminal => {
            let r = radius as i32;
            let body = Rect::new(
                center_x - r,
                center_y - r * 3 / 4,
                radius * 2,
                radius * 3 / 2,
            );
            canvas.fill_rounded_rect(body, (scale / 10).max(3), color.with_alpha(18));
            canvas.stroke_rounded_rect(body, (scale / 10).max(3), stroke.max(1), color);
            canvas.line(
                (center_x - r * 2 / 3, center_y - r / 4),
                (center_x - r / 4, center_y),
                stroke.max(2),
                color,
            );
            canvas.line(
                (center_x - r / 4, center_y),
                (center_x - r * 2 / 3, center_y + r / 4),
                stroke.max(2),
                color,
            );
            canvas.line(
                (center_x, center_y + r / 4),
                (center_x + r * 2 / 3, center_y + r / 4),
                stroke.max(2),
                color.with_alpha(220),
            );
        }
        Icon::Browser => {
            let pi = std::f32::consts::PI;
            let rr = radius as f32;
            draw_arc(
                canvas,
                (center_x, center_y),
                rr,
                (0.0, pi * 2.0),
                40,
                stroke.max(2),
                color,
            );
            draw_arc(
                canvas,
                (center_x, center_y),
                rr * 0.55,
                (pi * 0.5, pi * 1.5),
                22,
                stroke.max(1),
                color.with_alpha(190),
            );
            draw_arc(
                canvas,
                (center_x, center_y),
                rr * 0.55,
                (-pi * 0.5, pi * 0.5),
                22,
                stroke.max(1),
                color.with_alpha(190),
            );
            canvas.line(
                (center_x - radius as i32, center_y),
                (center_x + radius as i32, center_y),
                stroke.max(1),
                color.with_alpha(210),
            );
            canvas.line(
                (center_x, center_y - radius as i32),
                (center_x, center_y + radius as i32),
                stroke.max(1),
                color.with_alpha(180),
            );
        }
        Icon::Settings => {
            let hub = (scale / 7).max(3);
            let outer = radius as i32;
            canvas.circle(center_x, center_y, (scale / 5).max(4), color.with_alpha(35));
            draw_arc(
                canvas,
                (center_x, center_y),
                radius as f32 * 0.66,
                (0.0, std::f32::consts::PI * 2.0),
                32,
                stroke.max(2),
                color,
            );
            canvas.circle(center_x, center_y, hub, color.with_alpha(80));
            for index in 0..8 {
                let angle = index as f32 * std::f32::consts::PI / 4.0;
                let inner = (
                    (center_x as f32 + radius as f32 * 0.72 * angle.cos()).round() as i32,
                    (center_y as f32 + radius as f32 * 0.72 * angle.sin()).round() as i32,
                );
                let outer_pt = (
                    (center_x as f32 + outer as f32 * 1.05 * angle.cos()).round() as i32,
                    (center_y as f32 + outer as f32 * 1.05 * angle.sin()).round() as i32,
                );
                canvas.line(inner, outer_pt, stroke.max(2), color);
            }
        }
        Icon::Media => {
            draw_arc(
                canvas,
                (center_x, center_y),
                radius as f32,
                (0.0, std::f32::consts::PI * 2.0),
                36,
                stroke.max(2),
                color,
            );
            let r = radius as i32;
            let left = center_x - r / 3;
            canvas.line(
                (left, center_y - r / 2),
                (left, center_y + r / 2),
                stroke.max(2),
                color,
            );
            canvas.line(
                (left, center_y - r / 2),
                (center_x + r / 2, center_y),
                stroke.max(2),
                color,
            );
            canvas.line(
                (center_x + r / 2, center_y),
                (left, center_y + r / 2),
                stroke.max(2),
                color,
            );
        }
        Icon::Recovery => {
            let pi = std::f32::consts::PI;
            draw_arc(
                canvas,
                (center_x, center_y),
                radius as f32,
                (pi * 0.20, pi * 1.82),
                34,
                stroke.max(2),
                color,
            );
            let r = radius as i32;
            canvas.line(
                (center_x + r, center_y - r / 3),
                (center_x + r / 2, center_y - r / 2),
                stroke.max(2),
                color,
            );
            canvas.line(
                (center_x + r, center_y - r / 3),
                (center_x + r * 4 / 5, center_y + r / 5),
                stroke.max(2),
                color,
            );
        }
        Icon::Shield => {
            let r = radius as i32;
            let points = [
                (center_x, center_y - r),
                (center_x - r, center_y - r / 2),
                (center_x - r * 3 / 4, center_y + r / 2),
                (center_x, center_y + r),
                (center_x + r * 3 / 4, center_y + r / 2),
                (center_x + r, center_y - r / 2),
                (center_x, center_y - r),
            ];
            for pair in points.windows(2) {
                canvas.line(pair[0], pair[1], stroke.max(1), color);
            }
            canvas.circle(center_x, center_y, stroke.max(1), color.with_alpha(220));
        }
        Icon::Wifi => {
            let pi = std::f32::consts::PI;
            let anchor_y = center_y + radius as i32 / 2;
            draw_arc(
                canvas,
                (center_x, anchor_y),
                radius as f32,
                (pi * 1.12, pi * 1.88),
                18,
                stroke.max(1),
                color.with_alpha(215),
            );
            draw_arc(
                canvas,
                (center_x, anchor_y),
                radius as f32 * 0.66,
                (pi * 1.12, pi * 1.88),
                14,
                stroke.max(1),
                color.with_alpha(230),
            );
            canvas.circle(center_x, anchor_y, stroke.max(1), color);
        }
        Icon::Battery => {
            let r = radius as i32;
            let body = Rect::new(
                center_x - r,
                center_y - r / 2,
                (radius * 2).max(8),
                radius.max(5),
            );
            canvas.stroke_rounded_rect(body, 2, stroke.max(1), color);
            canvas.fill_rounded_rect(
                Rect::new(
                    center_x + r + 1,
                    center_y - (r / 5).max(1),
                    stroke.max(2),
                    (radius / 3).max(3),
                ),
                1,
                color.with_alpha(210),
            );
        }
        Icon::Status => {
            for offset in [-1, 0, 1] {
                let y = center_y + offset * (scale as i32 / 5).max(3);
                canvas.line(
                    (center_x - radius as i32, y),
                    (center_x + radius as i32, y),
                    stroke,
                    color.with_alpha(180),
                );
                canvas.circle(center_x + offset * 4, y, stroke.max(1), color);
            }
        }
        Icon::Network => {
            let bottom = center_y + radius as i32 / 2;
            canvas.circle(center_x, bottom, stroke.max(1), color);
            canvas.line(
                (center_x - radius as i32, center_y - radius as i32 / 2),
                (center_x, bottom - 3),
                stroke,
                color.with_alpha(150),
            );
            canvas.line(
                (center_x + radius as i32, center_y - radius as i32 / 2),
                (center_x, bottom - 3),
                stroke,
                color.with_alpha(150),
            );
            canvas.line(
                (center_x - radius as i32 / 2, center_y),
                (center_x + radius as i32 / 2, center_y),
                stroke,
                color,
            );
        }
        Icon::Audio => {
            let body = (scale / 5).max(3);
            canvas.fill_rounded_rect(
                Rect::new(
                    center_x - radius as i32,
                    center_y - body as i32 / 2,
                    body,
                    body,
                ),
                1,
                color,
            );
            canvas.line(
                (center_x - radius as i32 + body as i32, center_y),
                (center_x, center_y - radius as i32 / 2),
                stroke,
                color,
            );
            canvas.line(
                (center_x - radius as i32 + body as i32, center_y),
                (center_x, center_y + radius as i32 / 2),
                stroke,
                color,
            );
            canvas.line(
                (center_x + 3, center_y - radius as i32 / 2),
                (center_x + radius as i32, center_y),
                stroke,
                color.with_alpha(180),
            );
            canvas.line(
                (center_x + radius as i32, center_y),
                (center_x + 3, center_y + radius as i32 / 2),
                stroke,
                color.with_alpha(180),
            );
        }
        Icon::Storage => {
            canvas.stroke_rounded_rect(
                Rect::new(
                    center_x - radius as i32,
                    center_y - radius as i32 / 2,
                    radius * 2,
                    radius,
                ),
                (scale / 10).max(2),
                stroke,
                color,
            );
            canvas.circle(center_x + radius as i32 / 2, center_y, stroke.max(1), color);
        }
        Icon::Health => {
            let r = radius as i32;
            let top_y = center_y - r / 3;
            draw_arc(
                canvas,
                (center_x - r / 2, top_y),
                radius as f32 * 0.58,
                (std::f32::consts::PI * 0.95, std::f32::consts::PI * 2.02),
                20,
                stroke.max(2),
                color,
            );
            draw_arc(
                canvas,
                (center_x + r / 2, top_y),
                radius as f32 * 0.58,
                (std::f32::consts::PI * 0.98, std::f32::consts::PI * 2.05),
                20,
                stroke.max(2),
                color,
            );
            canvas.line(
                (center_x - r, top_y),
                (center_x, center_y + r),
                stroke.max(2),
                color,
            );
            canvas.line(
                (center_x + r, top_y),
                (center_x, center_y + r),
                stroke.max(2),
                color,
            );
            canvas.line(
                (center_x - r * 3 / 4, center_y),
                (center_x - r / 3, center_y),
                stroke.max(1),
                color.with_alpha(230),
            );
            canvas.line(
                (center_x - r / 3, center_y),
                (center_x, center_y - r / 3),
                stroke.max(1),
                color.with_alpha(230),
            );
            canvas.line(
                (center_x, center_y - r / 3),
                (center_x + r / 3, center_y + r / 3),
                stroke.max(1),
                color.with_alpha(230),
            );
            canvas.line(
                (center_x + r / 3, center_y + r / 3),
                (center_x + r * 3 / 4, center_y),
                stroke.max(1),
                color.with_alpha(230),
            );
        }
        Icon::Restart => {
            canvas.line(
                (center_x - radius as i32, center_y),
                (center_x - radius as i32 / 2, center_y - radius as i32),
                stroke,
                color,
            );
            canvas.line(
                (center_x - radius as i32 / 2, center_y - radius as i32),
                (center_x + radius as i32 / 2, center_y - radius as i32),
                stroke,
                color,
            );
            canvas.line(
                (center_x + radius as i32 / 2, center_y - radius as i32),
                (center_x + radius as i32, center_y),
                stroke,
                color,
            );
            canvas.line(
                (center_x + radius as i32, center_y),
                (center_x + radius as i32 / 3, center_y + radius as i32),
                stroke,
                color.with_alpha(190),
            );
            canvas.line(
                (center_x - radius as i32, center_y),
                (center_x - radius as i32 + 5, center_y - 5),
                stroke,
                color,
            );
        }
        Icon::Power => {
            canvas.circle(center_x, center_y + 2, radius, color.with_alpha(90));
            canvas.line(
                (center_x, center_y - radius as i32 - 2),
                (center_x, center_y + 2),
                stroke.max(2),
                color,
            );
        }
        Icon::Search => {
            let ring_radius = (scale as f32 * 0.27).max(4.0);
            let ring_x = center_x - (scale as i32 / 10);
            let ring_y = center_y - (scale as i32 / 10);
            draw_arc(
                canvas,
                (ring_x, ring_y),
                ring_radius,
                (0.0, std::f32::consts::PI * 2.0),
                28,
                stroke.max(2),
                color,
            );
            let handle_start = (
                (ring_x as f32 + ring_radius * 0.70).round() as i32,
                (ring_y as f32 + ring_radius * 0.70).round() as i32,
            );
            canvas.line(
                handle_start,
                (center_x + radius as i32, center_y + radius as i32),
                stroke.max(2),
                color,
            );
        }
        Icon::Chevron => {
            canvas.line(
                (center_x - radius as i32 / 2, center_y - radius as i32),
                (center_x + radius as i32 / 2, center_y),
                stroke,
                color,
            );
            canvas.line(
                (center_x + radius as i32 / 2, center_y),
                (center_x - radius as i32 / 2, center_y + radius as i32),
                stroke,
                color,
            );
        }
    }
}
