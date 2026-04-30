use std::sync::Arc;

use cosmic_text::{Buffer, LayoutGlyph, Metrics, Wrap};

use crate::SharedString;
use crate::style::ResolvedTextStyle;

#[derive(Debug, Clone)]
pub struct TextLayout {
    pub width: f32,
    pub height: f32,
    pub runs: Vec<TextRun>,
}

#[derive(Debug, Clone)]
pub struct TextRun {
    pub baseline: f32,
    pub glyphs: Vec<LayoutGlyph>,
}

pub type SharedTextLayout = Arc<TextLayout>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextMeasureKey {
    pub text_hash: u64,
    pub style_hash: u64,
    pub width_bits: Option<u32>,
}

#[derive(Debug, Default)]
pub struct TextCacheStats {
    pub hits: u64,
    pub misses: u64,
}

impl TextCacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TextBlockRevision {
    pub(crate) text: u64,
    pub(crate) style: u64,
    pub(crate) width: u64,
    pub(crate) layout: u64,
}

impl TextBlockRevision {
    pub const fn new() -> Self {
        Self {
            text: 1,
            style: 1,
            width: 1,
            layout: 0,
        }
    }
}

impl Default for TextBlockRevision {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TextBlockShapeKey {
    pub(crate) text_hash: u64,
    pub(crate) style_hash: u64,
    pub(crate) width_bits: Option<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct TextBlock {
    pub(crate) text: SharedString,
    pub(crate) style: ResolvedTextStyle,
    pub(crate) width: Option<f32>,
    pub(crate) revision: TextBlockRevision,
    pub(crate) shape_key: TextBlockShapeKey,
    pub(crate) buffer: Buffer,
    pub(crate) layout: SharedTextLayout,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TextBlockBufferConfig {
    pub(crate) metrics: Metrics,
    pub(crate) wrap: Wrap,
    pub(crate) width: Option<f32>,
    pub(crate) requested_width: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RasterGlyphFormat {
    Mask,
    Rgba,
}

#[derive(Debug, Clone)]
pub(crate) struct RasterGlyph {
    pub(crate) placement_left: i32,
    pub(crate) placement_top: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: RasterGlyphFormat,
    pub(crate) bytes: Vec<u8>,
}
