use std::{collections::HashMap, error::Error, fmt};

use fontdb::{Database, Family, Query, Stretch, Style, Weight};
use fontdue::{Font, FontSettings, Metrics};

use super::primitives::{Argb, Canvas};

const MAX_CACHED_GLYPHS: usize = 512;

pub(crate) const fn preferred_families() -> [&'static str; 2] {
    ["Noto Sans", "DejaVu Sans"]
}

pub(crate) const fn coverage_color(color: Argb, coverage: u8) -> Argb {
    let alpha = ((color.a as u16 * coverage as u16 + 127) / 255) as u8;
    if alpha == 0 {
        Argb::TRANSPARENT
    } else {
        Argb { a: alpha, ..color }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum FontWeight {
    Regular,
    Semibold,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextStyle {
    pub(crate) size_px: u16,
    pub(crate) weight: FontWeight,
}

impl TextStyle {
    pub(crate) const fn body() -> Self {
        Self {
            size_px: 16,
            weight: FontWeight::Regular,
        }
    }

    pub(crate) const fn title() -> Self {
        Self {
            size_px: 24,
            weight: FontWeight::Semibold,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TextMetrics {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug)]
pub(crate) enum TextError {
    NoUsableFont,
    InvalidFont(String),
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUsableFont => {
                formatter.write_str("Prime Shell could not find Noto Sans or DejaVu Sans")
            }
            Self::InvalidFont(message) => write!(
                formatter,
                "Prime Shell could not parse system font: {message}"
            ),
        }
    }
}

impl Error for TextError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct GlyphKey {
    character: char,
    size_px: u16,
    weight: FontWeight,
}

#[derive(Clone)]
struct CachedGlyph {
    metrics: Metrics,
    bitmap: Vec<u8>,
}

pub(crate) struct TextSystem {
    family_name: String,
    regular: Font,
    semibold: Font,
    cache: HashMap<GlyphKey, CachedGlyph>,
}

impl TextSystem {
    pub(crate) fn load_system() -> Result<Self, TextError> {
        let mut database = Database::new();
        database.load_system_fonts();

        for family_name in preferred_families() {
            let Some(regular) = load_face(&database, family_name, Weight::NORMAL)? else {
                continue;
            };
            let semibold = load_face(&database, family_name, Weight::SEMIBOLD)?
                .unwrap_or_else(|| regular.clone());
            return Ok(Self {
                family_name: family_name.to_owned(),
                regular,
                semibold,
                cache: HashMap::new(),
            });
        }

        Err(TextError::NoUsableFont)
    }

    pub(crate) fn family_name(&self) -> &str {
        &self.family_name
    }

    pub(crate) fn measure(&self, text: &str, style: TextStyle) -> TextMetrics {
        let font = self.font(style.weight);
        let size = f32::from(style.size_px);
        let width = text
            .chars()
            .map(|character| font.metrics(character, size).advance_width)
            .sum::<f32>()
            .ceil()
            .max(0.0) as u32;
        let height = font
            .horizontal_line_metrics(size)
            .map(|metrics| metrics.new_line_size.ceil().max(0.0) as u32)
            .unwrap_or_else(|| {
                text.chars()
                    .map(|character| font.metrics(character, size).height as u32)
                    .max()
                    .unwrap_or(0)
            });
        TextMetrics { width, height }
    }

    pub(crate) fn draw(
        &mut self,
        canvas: &mut Canvas<'_>,
        origin: (i32, i32),
        text: &str,
        style: TextStyle,
        color: Argb,
    ) {
        let size = f32::from(style.size_px);
        let ascent = self
            .font(style.weight)
            .horizontal_line_metrics(size)
            .map(|metrics| metrics.ascent)
            .unwrap_or(size);
        let baseline = origin.1.saturating_add(ascent.ceil() as i32);
        let mut cursor_x = origin.0 as f32;

        for character in text.chars() {
            let key = GlyphKey {
                character,
                size_px: style.size_px,
                weight: style.weight,
            };
            if !self.cache.contains_key(&key) {
                if self.cache.len() >= MAX_CACHED_GLYPHS {
                    self.cache.clear();
                }
                let (metrics, bitmap) = self.font(style.weight).rasterize(character, size);
                self.cache.insert(key, CachedGlyph { metrics, bitmap });
            }
            let glyph = self
                .cache
                .get(&key)
                .expect("Prime glyph cache entry disappeared after insertion");
            let glyph_x = cursor_x.round() as i32 + glyph.metrics.xmin;
            let glyph_y = baseline - glyph.metrics.ymin - glyph.metrics.height as i32;
            for row in 0..glyph.metrics.height {
                for column in 0..glyph.metrics.width {
                    let coverage = glyph.bitmap[row * glyph.metrics.width + column];
                    if coverage == 0 {
                        continue;
                    }
                    canvas.blend_pixel(
                        glyph_x.saturating_add(column as i32),
                        glyph_y.saturating_add(row as i32),
                        coverage_color(color, coverage),
                    );
                }
            }
            cursor_x += glyph.metrics.advance_width;
        }
    }

    fn font(&self, weight: FontWeight) -> &Font {
        match weight {
            FontWeight::Regular => &self.regular,
            FontWeight::Semibold => &self.semibold,
        }
    }
}

fn load_face(
    database: &Database,
    family_name: &str,
    weight: Weight,
) -> Result<Option<Font>, TextError> {
    let families = [Family::Name(family_name)];
    let Some(id) = database.query(&Query {
        families: &families,
        weight,
        stretch: Stretch::Normal,
        style: Style::Normal,
    }) else {
        return Ok(None);
    };
    let Some((bytes, collection_index)) =
        database.with_face_data(id, |data, index| (data.to_vec(), index))
    else {
        return Ok(None);
    };
    Font::from_bytes(
        bytes,
        FontSettings {
            collection_index,
            ..FontSettings::default()
        },
    )
    .map(Some)
    .map_err(|error| TextError::InvalidFont(error.to_string()))
}
