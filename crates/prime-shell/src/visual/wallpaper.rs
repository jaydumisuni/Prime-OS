use std::{fs, io::Cursor, path::Path};

use serde_json::{json, Value};

use super::{Argb, Canvas};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SystemWallpaper {
    pub(crate) id: &'static str,
    pub(crate) title: &'static str,
    pub(crate) encoded: &'static [u8],
}

pub(crate) const SYSTEM_WALLPAPERS: [SystemWallpaper; 8] = [
    SystemWallpaper {
        id: "system-01",
        title: "Prime 01",
        encoded: include_bytes!("../../assets/wallpapers/Prime_OS_Wallpaper_01.png"),
    },
    SystemWallpaper {
        id: "system-02",
        title: "Prime 02",
        encoded: include_bytes!("../../assets/wallpapers/Prime_OS_Wallpaper_02.png"),
    },
    SystemWallpaper {
        id: "system-03",
        title: "Prime 03",
        encoded: include_bytes!("../../assets/wallpapers/Prime_OS_Wallpaper_03.png"),
    },
    SystemWallpaper {
        id: "system-04",
        title: "Prime 04",
        encoded: include_bytes!("../../assets/wallpapers/Prime_OS_Wallpaper_04.png"),
    },
    SystemWallpaper {
        id: "system-05",
        title: "Prime OS 05",
        encoded: include_bytes!("../../assets/wallpapers/Prime_OS_Wallpaper_05_PRIME_OS.png"),
    },
    SystemWallpaper {
        id: "system-06",
        title: "Prime OS 06",
        encoded: include_bytes!("../../assets/wallpapers/Prime_OS_Wallpaper_06_PRIME_OS.png"),
    },
    SystemWallpaper {
        id: "system-07",
        title: "Prime OS 07",
        encoded: include_bytes!("../../assets/wallpapers/Prime_OS_Wallpaper_07_PRIME_OS.png"),
    },
    SystemWallpaper {
        id: "system-08",
        title: "Prime OS 08",
        encoded: include_bytes!("../../assets/wallpapers/Prime_OS_Wallpaper_08_PRIME_OS.png"),
    },
];

pub(crate) fn system_wallpapers() -> &'static [SystemWallpaper] {
    &SYSTEM_WALLPAPERS
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedWallpaper {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

pub(crate) fn decode_system_wallpaper(index: usize) -> Result<DecodedWallpaper, String> {
    let entry = SYSTEM_WALLPAPERS
        .get(index)
        .ok_or_else(|| format!("unknown Prime wallpaper index {index}"))?;
    decode_png(entry.encoded)
}

fn decode_png(encoded: &[u8]) -> Result<DecodedWallpaper, String> {
    let mut decoder = png::Decoder::new(Cursor::new(encoded));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    let source = &buffer[..info.buffer_size()];
    let pixel_count = info.width as usize * info.height as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);

    match info.color_type {
        png::ColorType::Rgb => {
            for pixel in source.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::Rgba => rgba.extend_from_slice(source),
        png::ColorType::Grayscale => {
            for &value in source {
                rgba.extend_from_slice(&[value, value, value, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in source.chunks_exact(2) {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Indexed => {
            return Err("expanded Prime wallpaper unexpectedly remained indexed".to_owned());
        }
    }

    if rgba.len() != pixel_count * 4 {
        return Err("decoded Prime wallpaper has an unexpected pixel buffer size".to_owned());
    }

    Ok(DecodedWallpaper {
        width: info.width,
        height: info.height,
        rgba,
    })
}

pub(crate) fn paint_system_wallpaper(canvas: &mut Canvas<'_>, wallpaper: &DecodedWallpaper) {
    if canvas.width == 0
        || canvas.height == 0
        || wallpaper.width == 0
        || wallpaper.height == 0
        || wallpaper.rgba.len() != wallpaper.width as usize * wallpaper.height as usize * 4
    {
        return;
    }

    let output_ratio = canvas.width as f32 / canvas.height as f32;
    let source_ratio = wallpaper.width as f32 / wallpaper.height as f32;
    let (crop_x, crop_y, crop_width, crop_height) = if source_ratio > output_ratio {
        let visible_width = wallpaper.height as f32 * output_ratio;
        (
            (wallpaper.width as f32 - visible_width) * 0.5,
            0.0,
            visible_width,
            wallpaper.height as f32,
        )
    } else {
        let visible_height = wallpaper.width as f32 / output_ratio;
        (
            0.0,
            (wallpaper.height as f32 - visible_height) * 0.5,
            wallpaper.width as f32,
            visible_height,
        )
    };

    let source_max_x = wallpaper.width.saturating_sub(1) as f32;
    let source_max_y = wallpaper.height.saturating_sub(1) as f32;
    let target_max_x = canvas.width.saturating_sub(1).max(1) as f32;
    let target_max_y = canvas.height.saturating_sub(1).max(1) as f32;

    for y in 0..canvas.height {
        let sy = (crop_y + y as f32 / target_max_y * (crop_height - 1.0)).clamp(0.0, source_max_y);
        for x in 0..canvas.width {
            let sx =
                (crop_x + x as f32 / target_max_x * (crop_width - 1.0)).clamp(0.0, source_max_x);
            canvas.blend_pixel(x as i32, y as i32, bilinear_pixel(wallpaper, sx, sy));
        }
    }
}

fn bilinear_pixel(wallpaper: &DecodedWallpaper, x: f32, y: f32) -> Argb {
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(wallpaper.width - 1);
    let y1 = (y0 + 1).min(wallpaper.height - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let p00 = rgba_pixel(wallpaper, x0, y0);
    let p10 = rgba_pixel(wallpaper, x1, y0);
    let p01 = rgba_pixel(wallpaper, x0, y1);
    let p11 = rgba_pixel(wallpaper, x1, y1);

    let mix = |a: u8, b: u8, t: f32| -> u8 {
        (f32::from(a) + (f32::from(b) - f32::from(a)) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    let row = |left: Argb, right: Argb| Argb {
        a: mix(left.a, right.a, tx),
        r: mix(left.r, right.r, tx),
        g: mix(left.g, right.g, tx),
        b: mix(left.b, right.b, tx),
    };
    let top = row(p00, p10);
    let bottom = row(p01, p11);
    Argb {
        a: mix(top.a, bottom.a, ty),
        r: mix(top.r, bottom.r, ty),
        g: mix(top.g, bottom.g, ty),
        b: mix(top.b, bottom.b, ty),
    }
}

fn rgba_pixel(wallpaper: &DecodedWallpaper, x: u32, y: u32) -> Argb {
    let offset = ((y * wallpaper.width + x) * 4) as usize;
    Argb {
        r: wallpaper.rgba[offset],
        g: wallpaper.rgba[offset + 1],
        b: wallpaper.rgba[offset + 2],
        a: wallpaper.rgba[offset + 3],
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum WallpaperSelection {
    #[default]
    Animated,
    System(usize),
}

impl WallpaperSelection {
    pub(crate) fn from_json(source: &str) -> Self {
        let Ok(value) = serde_json::from_str::<Value>(source) else {
            return Self::Animated;
        };
        if value
            .get("schema")
            .and_then(Value::as_str)
            .is_some_and(|schema| schema != "prime.wallpaper.v1")
        {
            return Self::Animated;
        }
        let Some(selection) = value.get("selection").and_then(Value::as_str) else {
            return Self::Animated;
        };
        if selection == "animated" {
            return Self::Animated;
        }
        SYSTEM_WALLPAPERS
            .iter()
            .position(|entry| entry.id == selection)
            .map(Self::System)
            .unwrap_or(Self::Animated)
    }

    pub(crate) fn to_json(self) -> Result<String, serde_json::Error> {
        let selection = match self {
            Self::Animated => "animated",
            Self::System(index) => SYSTEM_WALLPAPERS
                .get(index)
                .map(|entry| entry.id)
                .unwrap_or("animated"),
        };
        serde_json::to_string_pretty(&json!({
            "schema": "prime.wallpaper.v1",
            "selection": selection,
        }))
    }

    pub(crate) fn load_from_path(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .map(|source| Self::from_json(&source))
            .unwrap_or_default()
    }

    pub(crate) fn save_to_path(self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let encoded = self.to_json().map_err(|error| error.to_string())?;
        fs::write(path, encoded).map_err(|error| error.to_string())
    }
}
