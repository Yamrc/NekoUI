use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use cosmic_text::{Attrs, Buffer, Family, FontSystem, LayoutGlyph, Metrics, Shaping};

use crate::SharedString;
use crate::style::TextStyle;

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextMeasureKey {
    pub text_hash: u64,
    pub style_hash: u64,
    pub width_bits: Option<u32>,
}

#[derive(Debug)]
pub struct TextSystem {
    font_system: FontSystem,
}

impl TextSystem {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
        }
    }

    pub fn measure(
        &mut self,
        text: &SharedString,
        style: &TextStyle,
        width: Option<f32>,
    ) -> TextLayout {
        let metrics = Metrics::new(
            style.font_size,
            style.line_height.unwrap_or(style.font_size * 1.2),
        );
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, width, None);

        let mut attrs = Attrs::new();
        if let Some(family) = style.font_family.as_deref() {
            attrs = attrs.family(Family::Name(family));
        }

        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut runs = Vec::new();
        let mut width_px = 0.0_f32;
        let mut height_px = 0.0_f32;

        for run in buffer.layout_runs() {
            width_px = width_px.max(run.line_w);
            height_px = height_px.max(run.line_top + run.line_height);
            runs.push(TextRun {
                baseline: run.line_y,
                glyphs: run.glyphs.to_vec(),
            });
        }

        TextLayout {
            width: width_px,
            height: height_px,
            runs,
        }
    }

    pub(crate) fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

pub fn measure_key(text: &SharedString, style: &TextStyle, width: Option<f32>) -> TextMeasureKey {
    TextMeasureKey {
        text_hash: hash_value(text.as_ref()),
        style_hash: hash_text_style(style),
        width_bits: width.map(f32::to_bits),
    }
}

fn hash_text_style(style: &TextStyle) -> u64 {
    let mut hasher = DefaultHasher::new();
    style.font_family.hash(&mut hasher);
    style.font_size.to_bits().hash(&mut hasher);
    style.line_height.map(f32::to_bits).hash(&mut hasher);
    style.color.r.to_bits().hash(&mut hasher);
    style.color.g.to_bits().hash(&mut hasher);
    style.color.b.to_bits().hash(&mut hasher);
    style.color.a.to_bits().hash(&mut hasher);
    hasher.finish()
}

fn hash_value(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
