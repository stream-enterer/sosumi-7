use std::collections::HashMap;

use skrifa::instance::Size;
use skrifa::outline::{DrawSettings, HintingInstance, HintingOptions};
use skrifa::MetadataProvider;

static DEFAULT_FONT_DATA: &[u8] = include_bytes!("../../res/fonts/default.ttf");

/// Result of shaping a single glyph within a text run.
pub struct ShapedGlyph {
    pub glyph_id: u16,
    pub x_offset: f64,
    pub y_offset: f64,
    pub x_advance: f64,
}

/// Cache key for a rasterized glyph.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GlyphCacheKey {
    pub font_id: u16,
    pub size_px: u16,
    pub glyph_id: u16,
}

/// Rasterized glyph stored in the cache.
#[allow(dead_code)]
pub(crate) struct CachedGlyph {
    /// 1-channel greyscale alpha mask.
    pub bitmap: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Horizontal offset from pen position (pixels).
    pub bearing_x: i32,
    /// Vertical offset: distance from baseline to top of bitmap (positive = up).
    pub bearing_y: i32,
    pub advance: f64,
    last_used: u64,
    byte_size: usize,
}

struct LoadedFont {
    data: Vec<u8>,
    /// Cached hinting instances keyed by quantized size_px.
    hinting_instances: HashMap<u16, Option<HintingInstance>>,
}

/// Font cache with shaping (rustybuzz), hinted rasterization (skrifa+zeno),
/// and LRU glyph bitmap caching.
pub struct FontCache {
    fonts: Vec<LoadedFont>,
    glyph_cache: HashMap<GlyphCacheKey, CachedGlyph>,
    cache_byte_budget: usize,
    cache_bytes_used: usize,
    frame_counter: u64,
}

impl FontCache {
    /// Default text size in user-coordinate pixels (matches a standard UI font).
    pub const DEFAULT_SIZE_PX: f64 = 13.0;

    pub fn new() -> Self {
        let mut cache = Self {
            fonts: Vec::new(),
            glyph_cache: HashMap::new(),
            cache_byte_budget: 16 * 1024 * 1024,
            cache_bytes_used: 0,
            frame_counter: 0,
        };
        cache.fonts.push(LoadedFont {
            data: DEFAULT_FONT_DATA.to_vec(),
            hinting_instances: HashMap::new(),
        });
        cache
    }

    /// Register a user-supplied font. Returns the font_id for later use.
    pub fn add_font(&mut self, data: Vec<u8>) -> u16 {
        let id = self.fonts.len() as u16;
        self.fonts.push(LoadedFont {
            data,
            hinting_instances: HashMap::new(),
        });
        id
    }

    /// Quantize an effective pixel size to an integer for cache keying.
    /// Sizes <= 48 round to nearest integer; larger sizes round to nearest even.
    pub fn quantize_size(effective_px: f64) -> u16 {
        let px = effective_px.round().max(1.0) as u16;
        if px > 48 {
            (px + 1) & !1
        } else {
            px
        }
    }

    /// Measure text dimensions in pixels at the given size.
    /// Returns (width, height). Shaping is performed on the fly.
    pub fn measure_text(&self, text: &str, font_id: u16, size_px: u16) -> (f64, f64) {
        if text.is_empty() || size_px < 2 {
            return (0.0, 0.0);
        }
        let shaped = self.shape_text(text, font_id, size_px);
        let width = shaped.iter().map(|g| g.x_advance).sum::<f64>();
        (width, size_px as f64)
    }

    /// Shape text using rustybuzz. Returns positioned glyph IDs with offsets.
    pub fn shape_text(&self, text: &str, font_id: u16, size_px: u16) -> Vec<ShapedGlyph> {
        let font = match self.fonts.get(font_id as usize) {
            Some(f) => f,
            None => return Vec::new(),
        };

        let face = match rustybuzz::Face::from_slice(&font.data, 0) {
            Some(f) => f,
            None => return Vec::new(),
        };

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);

        let output = rustybuzz::shape(&face, &[], buffer);

        let upem = face.units_per_em() as f64;
        let scale = size_px as f64 / upem;

        let infos = output.glyph_infos();
        let positions = output.glyph_positions();

        infos
            .iter()
            .zip(positions.iter())
            .map(|(info, pos)| ShapedGlyph {
                glyph_id: info.glyph_id as u16,
                x_offset: pos.x_offset as f64 * scale,
                y_offset: pos.y_offset as f64 * scale,
                x_advance: pos.x_advance as f64 * scale,
            })
            .collect()
    }

    /// Ensure a glyph is rasterized and in the cache. No-op if already cached.
    pub fn ensure_glyph(&mut self, font_id: u16, size_px: u16, glyph_id: u16) {
        let key = GlyphCacheKey {
            font_id,
            size_px,
            glyph_id,
        };
        if let Some(g) = self.glyph_cache.get_mut(&key) {
            g.last_used = self.frame_counter;
            return;
        }

        self.rasterize_glyph(key);
    }

    /// Look up a cached glyph (immutable — for use during rendering).
    pub(crate) fn get_cached_glyph(&self, key: &GlyphCacheKey) -> Option<&CachedGlyph> {
        self.glyph_cache.get(key)
    }

    /// Get the font ascent in pixels at the given size.
    pub fn ascent(&self, font_id: u16, size_px: u16) -> i32 {
        let font = match self.fonts.get(font_id as usize) {
            Some(f) => f,
            None => return size_px as i32,
        };
        let font_ref = match skrifa::FontRef::new(&font.data) {
            Ok(f) => f,
            Err(_) => return size_px as i32,
        };
        let size = Size::new(size_px as f32);
        let metrics = font_ref.metrics(size, skrifa::instance::LocationRef::default());
        metrics.ascent.round() as i32
    }

    /// Get the line height in user coordinates at the given size.
    pub fn line_height(&self, font_id: u16, size_px: u16) -> f64 {
        let font = match self.fonts.get(font_id as usize) {
            Some(f) => f,
            None => return size_px as f64,
        };
        let font_ref = match skrifa::FontRef::new(&font.data) {
            Ok(f) => f,
            Err(_) => return size_px as f64,
        };
        let size = Size::new(size_px as f32);
        let metrics = font_ref.metrics(size, skrifa::instance::LocationRef::default());
        (metrics.ascent + metrics.descent.abs() + metrics.leading) as f64
    }

    /// Increment the frame counter (call once per frame).
    pub fn advance_frame(&mut self) {
        self.frame_counter += 1;
    }

    fn rasterize_glyph(&mut self, key: GlyphCacheKey) {
        let font = match self.fonts.get_mut(key.font_id as usize) {
            Some(f) => f,
            None => return,
        };
        let font_ref = match skrifa::FontRef::new(&font.data) {
            Ok(f) => f,
            Err(_) => return,
        };

        let size = Size::new(key.size_px as f32);
        let location = skrifa::instance::LocationRef::default();
        let glyph_id = skrifa::GlyphId::new(key.glyph_id as u32);

        let glyph_metrics = font_ref.glyph_metrics(size, location);
        let advance = glyph_metrics.advance_width(glyph_id).unwrap_or(0.0) as f64;

        let outlines = font_ref.outline_glyphs();
        let outline = match outlines.get(glyph_id) {
            Some(o) => o,
            None => {
                // No outline (space, control char) — cache empty glyph with advance.
                self.insert_empty_glyph(key, advance);
                return;
            }
        };

        // Get or create a hinting instance for this size.
        let hinting = font
            .hinting_instances
            .entry(key.size_px)
            .or_insert_with(|| {
                HintingInstance::new(&outlines, size, location, HintingOptions::default()).ok()
            });

        let mut pen = ZenoPen {
            commands: Vec::new(),
        };
        let settings = match hinting {
            Some(inst) => DrawSettings::hinted(inst, false),
            None => DrawSettings::unhinted(size, location),
        };
        if outline.draw(settings, &mut pen).is_err() {
            self.insert_empty_glyph(key, advance);
            return;
        }

        if pen.commands.is_empty() {
            self.insert_empty_glyph(key, advance);
            return;
        }

        // Rasterize with zeno.
        let (data, placement) = zeno::Mask::new(&pen.commands[..])
            .format(zeno::Format::Alpha)
            .render();

        let byte_size = data.len();
        self.glyph_cache.insert(
            key,
            CachedGlyph {
                bitmap: data,
                width: placement.width,
                height: placement.height,
                bearing_x: placement.left,
                bearing_y: -placement.top, // zeno top is negative-y-up, we want positive = above baseline
                advance,
                last_used: self.frame_counter,
                byte_size,
            },
        );

        self.cache_bytes_used += byte_size;
        if self.cache_bytes_used > self.cache_byte_budget {
            self.evict_lru();
        }
    }

    fn insert_empty_glyph(&mut self, key: GlyphCacheKey, advance: f64) {
        self.glyph_cache.insert(
            key,
            CachedGlyph {
                bitmap: Vec::new(),
                width: 0,
                height: 0,
                bearing_x: 0,
                bearing_y: 0,
                advance,
                last_used: self.frame_counter,
                byte_size: 0,
            },
        );
    }

    fn evict_lru(&mut self) {
        let target = self.cache_byte_budget * 3 / 4;
        while self.cache_bytes_used > target {
            let oldest = self
                .glyph_cache
                .iter()
                .min_by_key(|(_, g)| g.last_used)
                .map(|(k, g)| (*k, g.byte_size));
            if let Some((key, size)) = oldest {
                self.glyph_cache.remove(&key);
                self.cache_bytes_used -= size;
            } else {
                break;
            }
        }
    }
}

impl Default for FontCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Bridge: skrifa OutlinePen → zeno path commands.
struct ZenoPen {
    commands: Vec<zeno::Command>,
}

impl skrifa::outline::OutlinePen for ZenoPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.commands.push(zeno::Command::MoveTo([x, y].into()));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.commands.push(zeno::Command::LineTo([x, y].into()));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.commands
            .push(zeno::Command::QuadTo([cx, cy].into(), [x, y].into()));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.commands.push(zeno::Command::CurveTo(
            [cx0, cy0].into(),
            [cx1, cy1].into(),
            [x, y].into(),
        ));
    }

    fn close(&mut self) {
        self.commands.push(zeno::Command::Close);
    }
}
