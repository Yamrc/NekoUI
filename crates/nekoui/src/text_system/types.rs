use std::sync::Arc;

use cosmic_text::LayoutGlyph;

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
