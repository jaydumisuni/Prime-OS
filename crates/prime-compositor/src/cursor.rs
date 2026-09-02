use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
                Kind,
            },
            gles::{GlesError, GlesRenderer},
        },
    },
    utils::{Logical, Physical, Point, Transform},
};

pub(crate) const CURSOR_WIDTH: u32 = 24;
pub(crate) const CURSOR_HEIGHT: u32 = 32;
pub(crate) const CURSOR_HOTSPOT: (i32, i32) = (2, 2);

pub(crate) struct CursorState {
    buffer: MemoryRenderBuffer,
}

impl CursorState {
    pub(crate) fn new() -> Self {
        let pixels = default_cursor_pixels();
        Self {
            buffer: MemoryRenderBuffer::from_slice(
                &pixels,
                Fourcc::Argb8888,
                (CURSOR_WIDTH as i32, CURSOR_HEIGHT as i32),
                1,
                Transform::Normal,
                None,
            ),
        }
    }

    pub(crate) fn render_element(
        &self,
        renderer: &mut GlesRenderer,
        location: Point<f64, Logical>,
    ) -> Result<MemoryRenderBufferRenderElement<GlesRenderer>, GlesError> {
        let location: Point<f64, Physical> = Point::from((
            location.x - f64::from(CURSOR_HOTSPOT.0),
            location.y - f64::from(CURSOR_HOTSPOT.1),
        ));
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            location,
            &self.buffer,
            None,
            None,
            None,
            Kind::Cursor,
        )
    }
}

pub(crate) fn default_cursor_pixels() -> Vec<u8> {
    let mut pixels = vec![0u8; CURSOR_WIDTH as usize * CURSOR_HEIGHT as usize * 4];
    for y in 0..CURSOR_HEIGHT {
        for x in 0..CURSOR_WIDTH {
            let color = cursor_pixel_color(x, y);
            let offset = ((y * CURSOR_WIDTH + x) * 4) as usize;
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }
    pixels
}

fn cursor_pixel_color(x: u32, y: u32) -> [u8; 4] {
    // Classic arrow silhouette with a compact stem. ARGB8888 is byte-addressed
    // as BGRA on little-endian hosts; grayscale keeps the shape unambiguous.
    let head_right = 2 + y / 2;
    let in_head = y <= 19 && x >= 1 && x <= head_right.min(12);
    let in_stem = (13..=28).contains(&y) && (7..=10).contains(&x);
    if !(in_head || in_stem) {
        return [0, 0, 0, 0];
    }

    let outline = x <= 2
        || (in_head && x >= head_right.saturating_sub(1))
        || y <= 2
        || (in_stem && (x == 7 || x == 10 || y >= 27));
    if outline {
        [12, 15, 24, 255]
    } else {
        [245, 248, 255, 255]
    }
}

#[cfg(test)]
fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * CURSOR_WIDTH + x) * 4) as usize;
    pixels[offset..offset + 4].try_into().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cursor_has_transparent_canvas_and_visible_arrow() {
        let pixels = default_cursor_pixels();
        assert_eq!(
            pixels.len(),
            CURSOR_WIDTH as usize * CURSOR_HEIGHT as usize * 4
        );
        assert_eq!(pixel(&pixels, CURSOR_WIDTH - 1, CURSOR_HEIGHT - 1)[3], 0);
        assert_eq!(pixel(&pixels, 1, 1)[3], 255);
        assert!(pixel(&pixels, 5, 10)[3] > 0);
    }

    #[test]
    fn cursor_hotspot_is_inside_visible_arrow() {
        let pixels = default_cursor_pixels();
        let p = pixel(&pixels, CURSOR_HOTSPOT.0 as u32, CURSOR_HOTSPOT.1 as u32);
        assert_eq!(p[3], 255);
    }
}
