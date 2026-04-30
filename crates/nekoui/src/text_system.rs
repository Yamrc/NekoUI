mod cache;
mod selector;
mod types;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cosmic_text::{Align, Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache, Wrap};
use swash::scale::image::Content as SwashContent;
use unicode_segmentation::UnicodeSegmentation;

use crate::SharedString;
use crate::style::{Absolute, Definite, ResolvedTextStyle, TextOverflow, WhiteSpace};

use self::cache::AdaptiveLruCache;
use self::selector::{
    ClusterFamilyIndexCacheKey, FamilyCandidateCacheKey, default_text_attrs, text_align, text_attrs,
};
pub(crate) use self::types::{
    RasterGlyph, RasterGlyphFormat, SharedTextLayout, TextBlock, TextBlockBufferConfig,
    TextBlockRevision, TextBlockShapeKey,
};
pub use self::types::{TextCacheStats, TextLayout, TextMeasureKey, TextRun};

const DEFAULT_REM_SIZE_PX: f32 = 16.0;
const MEASURE_CACHE_BASE_LIMIT: usize = 2_048;
const MEASURE_CACHE_MAX_LIMIT: usize = 16_384;
const FAMILY_CANDIDATE_CACHE_BASE_LIMIT: usize = 256;
const FAMILY_CANDIDATE_CACHE_MAX_LIMIT: usize = 2_048;
const CLUSTER_FAMILY_INDEX_CACHE_BASE_LIMIT: usize = 8_192;
const CLUSTER_FAMILY_INDEX_CACHE_MAX_LIMIT: usize = 65_536;

#[derive(Debug)]
pub struct TextSystem {
    font_system: FontSystem,
    measure_cache: AdaptiveLruCache<TextMeasureKey, SharedTextLayout>,
    family_candidate_cache:
        AdaptiveLruCache<FamilyCandidateCacheKey, Arc<[cosmic_text::fontdb::ID]>>,
    cluster_family_index_cache: AdaptiveLruCache<ClusterFamilyIndexCacheKey, Option<usize>>,
    swash_cache: SwashCache,
    cache_stats: TextCacheStats,
}

impl TextSystem {
    pub(crate) fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            measure_cache: AdaptiveLruCache::new(MEASURE_CACHE_BASE_LIMIT, MEASURE_CACHE_MAX_LIMIT),
            family_candidate_cache: AdaptiveLruCache::new(
                FAMILY_CANDIDATE_CACHE_BASE_LIMIT,
                FAMILY_CANDIDATE_CACHE_MAX_LIMIT,
            ),
            cluster_family_index_cache: AdaptiveLruCache::new(
                CLUSTER_FAMILY_INDEX_CACHE_BASE_LIMIT,
                CLUSTER_FAMILY_INDEX_CACHE_MAX_LIMIT,
            ),
            swash_cache: SwashCache::new(),
            cache_stats: TextCacheStats::default(),
        }
    }

    pub(crate) fn measure(
        &mut self,
        text: &SharedString,
        style: &ResolvedTextStyle,
        width: Option<f32>,
    ) -> SharedTextLayout {
        let key = measure_key(text, style, width);

        if let Some(cached) = self.measure_cache.get(&key).cloned() {
            self.cache_stats.hits += 1;
            return cached;
        }

        self.cache_stats.misses += 1;
        let block = self.new_text_block(text.clone(), style.clone(), width);
        let shared = block.layout.clone();
        self.measure_cache.insert(key, shared.clone());
        shared
    }

    pub(crate) fn new_text_block(
        &mut self,
        text: SharedString,
        style: ResolvedTextStyle,
        width: Option<f32>,
    ) -> TextBlock {
        let revision = TextBlockRevision::new();
        let shape_key = TextBlockShapeKey {
            text_hash: hash_value(text.as_ref()),
            style_hash: hash_text_shape_style(&style),
            width_bits: width.map(f32::to_bits),
        };
        let config = self.buffer_config(&style, width);
        let mut buffer = Buffer::new(&mut self.font_system, config.metrics);
        self.configure_buffer(&mut buffer, config);
        self.set_buffer_text(&mut buffer, text.as_ref(), &style, config.requested_width);
        let layout = Arc::new(self.collect_layout(&buffer));

        TextBlock {
            text,
            style,
            width,
            revision: TextBlockRevision {
                layout: 1,
                ..revision
            },
            shape_key,
            buffer,
            layout,
        }
    }

    pub(crate) fn sync_text_block(
        &mut self,
        block: &mut TextBlock,
        text: SharedString,
        style: ResolvedTextStyle,
        width: Option<f32>,
    ) {
        let next_shape_key = TextBlockShapeKey {
            text_hash: hash_value(text.as_ref()),
            style_hash: hash_text_shape_style(&style),
            width_bits: width.map(f32::to_bits),
        };
        let shape_changed = block.shape_key != next_shape_key;

        if block.text.as_ref() != text.as_ref() {
            block.text = text;
            block.revision.text = block.revision.text.saturating_add(1);
        }
        if block.style != style {
            block.style = style;
            block.revision.style = block.revision.style.saturating_add(1);
        }
        if block.width != width {
            block.width = width;
            block.revision.width = block.revision.width.saturating_add(1);
        }

        if !shape_changed {
            return;
        }

        let config = self.buffer_config(&block.style, block.width);
        self.configure_buffer(&mut block.buffer, config);
        self.set_buffer_text(
            &mut block.buffer,
            block.text.as_ref(),
            &block.style,
            config.requested_width,
        );
        block.layout = Arc::new(self.collect_layout(&block.buffer));
        block.shape_key = next_shape_key;
        block.revision.layout = block.revision.layout.saturating_add(1);
    }

    fn buffer_config(
        &self,
        style: &ResolvedTextStyle,
        width: Option<f32>,
    ) -> TextBlockBufferConfig {
        let font_size = resolve_absolute(style.font_size);
        let line_height = style
            .line_height
            .map(|line_height| resolve_definite(line_height, font_size))
            .unwrap_or(font_size * 1.2);
        let wrap = match style.white_space {
            WhiteSpace::Normal => Wrap::WordOrGlyph,
            WhiteSpace::Nowrap => Wrap::None,
        };
        let requested_width = width;
        let width = match style.white_space {
            WhiteSpace::Normal => requested_width,
            WhiteSpace::Nowrap => None,
        };

        TextBlockBufferConfig {
            metrics: Metrics::new(font_size, line_height),
            wrap,
            width,
            requested_width,
        }
    }

    fn configure_buffer(&mut self, buffer: &mut Buffer, config: TextBlockBufferConfig) {
        buffer.set_metrics(&mut self.font_system, config.metrics);
        buffer.set_wrap(&mut self.font_system, config.wrap);
        buffer.set_size(&mut self.font_system, config.width, None);
    }

    fn set_buffer_text(
        &mut self,
        buffer: &mut Buffer,
        text: &str,
        style: &ResolvedTextStyle,
        requested_width: Option<f32>,
    ) {
        let attrs = default_text_attrs(style);
        let spans = self.rich_text_spans(text, style);
        buffer.set_rich_text(
            &mut self.font_system,
            spans.iter().map(|(range, family_index)| {
                (
                    &text[range.clone()],
                    family_index
                        .and_then(|index| style.font_families.get(index))
                        .map_or_else(|| attrs.clone(), |family| text_attrs(style, family)),
                )
            }),
            &attrs,
            Shaping::Advanced,
            Some(text_align(style.text_align)),
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        if matches!(style.white_space, WhiteSpace::Nowrap)
            && matches!(style.text_overflow, Some(TextOverflow::Ellipsis))
            && let Some(max_width) = requested_width
            && current_layout_width(buffer) > max_width
        {
            let truncated =
                self.truncate_text_to_width(text, buffer.metrics(), &attrs, style, max_width);
            let spans = self.rich_text_spans(&truncated, style);
            buffer.set_rich_text(
                &mut self.font_system,
                spans.iter().map(|(range, family_index)| {
                    (
                        &truncated[range.clone()],
                        family_index
                            .and_then(|index| style.font_families.get(index))
                            .map_or_else(|| attrs.clone(), |family| text_attrs(style, family)),
                    )
                }),
                &attrs,
                Shaping::Advanced,
                Some(text_align(style.text_align)),
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
        }
    }

    fn collect_layout(&self, buffer: &Buffer) -> TextLayout {
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

    fn truncate_text_to_width(
        &mut self,
        text: &str,
        metrics: Metrics,
        attrs: &Attrs<'_>,
        style: &ResolvedTextStyle,
        max_width: f32,
    ) -> SharedString {
        const ELLIPSIS: &str = "…";

        let ellipsis_width = self.measure_single_line_width(ELLIPSIS, metrics, style, attrs);
        if ellipsis_width >= max_width {
            return SharedString::from(ELLIPSIS);
        }

        let grapheme_ends = text
            .grapheme_indices(true)
            .map(|(start, grapheme)| start + grapheme.len())
            .collect::<Vec<_>>();

        if grapheme_ends.is_empty() {
            return SharedString::from(ELLIPSIS);
        }

        let mut low = 0usize;
        let mut high = grapheme_ends.len();
        while low < high {
            let mid = (low + high).div_ceil(2);
            let end = grapheme_ends[mid - 1];
            let candidate = format!("{}{ELLIPSIS}", &text[..end]);
            if self.measure_single_line_width(&candidate, metrics, style, attrs) <= max_width {
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        if low == 0 {
            SharedString::from(ELLIPSIS)
        } else {
            let end = grapheme_ends[low - 1];
            SharedString::from(format!("{}{ELLIPSIS}", &text[..end]))
        }
    }

    fn measure_single_line_width(
        &mut self,
        text: &str,
        metrics: Metrics,
        style: &ResolvedTextStyle,
        attrs: &Attrs<'_>,
    ) -> f32 {
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        self.configure_buffer(
            &mut buffer,
            TextBlockBufferConfig {
                metrics,
                wrap: Wrap::None,
                width: None,
                requested_width: None,
            },
        );
        let spans = self.rich_text_spans(text, style);
        buffer.set_rich_text(
            &mut self.font_system,
            spans.iter().map(|(range, family_index)| {
                (
                    &text[range.clone()],
                    family_index
                        .and_then(|index| style.font_families.get(index))
                        .map_or_else(|| attrs.clone(), |family| text_attrs(style, family)),
                )
            }),
            attrs,
            Shaping::Advanced,
            Some(Align::Left),
        );
        buffer.shape_until_scroll(&mut self.font_system, false);
        current_layout_width(&buffer)
    }

    #[allow(dead_code)]
    pub(crate) fn cache_stats(&self) -> &TextCacheStats {
        &self.cache_stats
    }

    #[allow(dead_code)]
    pub(crate) fn clear_cache(&mut self) {
        self.measure_cache.clear();
        self.family_candidate_cache.clear();
        self.cluster_family_index_cache.clear();
        self.cache_stats = TextCacheStats::default();
    }

    pub(crate) fn raster_glyph(&mut self, cache_key: cosmic_text::CacheKey) -> Option<RasterGlyph> {
        self.raster_glyph_via_swash(cache_key)
    }

    fn raster_glyph_via_swash(&mut self, cache_key: cosmic_text::CacheKey) -> Option<RasterGlyph> {
        let font_system = &mut self.font_system;
        let swash_cache = &mut self.swash_cache;
        let image = swash_cache
            .get_image(font_system, cache_key)
            .as_ref()?
            .clone();

        match image.content {
            SwashContent::Mask => Some(RasterGlyph {
                placement_left: image.placement.left,
                placement_top: image.placement.top,
                width: image.placement.width,
                height: image.placement.height,
                format: RasterGlyphFormat::Mask,
                bytes: image.data,
            }),
            // We do not have a dedicated LCD text pipeline yet, but we must keep
            // subpixel glyphs renderable instead of dropping them entirely.
            // Collapse the per-subpixel coverages to a grayscale mask for now.
            SwashContent::SubpixelMask => Some(RasterGlyph {
                placement_left: image.placement.left,
                placement_top: image.placement.top,
                width: image.placement.width,
                height: image.placement.height,
                format: RasterGlyphFormat::Mask,
                bytes: subpixel_mask_to_alpha(&image.data),
            }),
            SwashContent::Color => Some(RasterGlyph {
                placement_left: image.placement.left,
                placement_top: image.placement.top,
                width: image.placement.width,
                height: image.placement.height,
                format: RasterGlyphFormat::Rgba,
                bytes: image.data,
            }),
        }
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn measure_key(
    text: &SharedString,
    style: &ResolvedTextStyle,
    width: Option<f32>,
) -> TextMeasureKey {
    TextMeasureKey {
        text_hash: hash_value(text.as_ref()),
        style_hash: hash_text_shape_style(style),
        width_bits: width.map(f32::to_bits),
    }
}

fn hash_text_shape_style(style: &ResolvedTextStyle) -> u64 {
    let mut hasher = DefaultHasher::new();
    style.font_families.hash(&mut hasher);
    hash_absolute(style.font_size, &mut hasher);
    hash_option_definite(style.line_height, &mut hasher);
    style.font_weight.hash(&mut hasher);
    style.font_style.hash(&mut hasher);
    style.text_align.hash(&mut hasher);
    style.white_space.hash(&mut hasher);
    style.text_overflow.hash(&mut hasher);
    hasher.finish()
}

fn current_layout_width(buffer: &Buffer) -> f32 {
    buffer
        .layout_runs()
        .map(|run| run.line_w)
        .fold(0.0_f32, f32::max)
}

fn resolve_absolute(value: Absolute) -> f32 {
    match value {
        Absolute::Px(px) => px.get(),
        Absolute::Rem(rem) => rem.to_px(crate::style::Px(DEFAULT_REM_SIZE_PX)).get(),
    }
}

fn resolve_definite(value: Definite, base_px: f32) -> f32 {
    value
        .to_px(
            crate::style::Px(base_px),
            crate::style::Px(DEFAULT_REM_SIZE_PX),
        )
        .get()
}

fn hash_absolute(value: Absolute, hasher: &mut DefaultHasher) {
    match value {
        Absolute::Px(px) => {
            0_u8.hash(hasher);
            px.get().to_bits().hash(hasher);
        }
        Absolute::Rem(rem) => {
            1_u8.hash(hasher);
            rem.get().to_bits().hash(hasher);
        }
    }
}

fn hash_option_definite(value: Option<Definite>, hasher: &mut DefaultHasher) {
    match value {
        Some(definite) => {
            1_u8.hash(hasher);
            hash_definite(definite, hasher);
        }
        None => 0_u8.hash(hasher),
    }
}

fn hash_definite(value: Definite, hasher: &mut DefaultHasher) {
    match value {
        Definite::Absolute(absolute) => {
            0_u8.hash(hasher);
            hash_absolute(absolute, hasher);
        }
        Definite::Percent(percent) => {
            1_u8.hash(hasher);
            percent.get().to_bits().hash(hasher);
        }
    }
}

fn hash_value(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn subpixel_mask_to_alpha(bytes: &[u8]) -> Vec<u8> {
    bytes
        .chunks_exact(4)
        .map(|pixel| ((u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 3) as u8)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::style::{Color, ResolvedTextStyle, TextOverflow, WhiteSpace};

    use super::{TextSystem, measure_key, subpixel_mask_to_alpha};

    #[test]
    fn measure_key_ignores_paint_only_text_color_changes() {
        let text = Arc::<str>::from("hello color");
        let old = ResolvedTextStyle::default();
        let mut new = old.clone();
        new.color = Color::rgb(0x336699);

        assert_eq!(
            measure_key(&text, &old, Some(240.0)),
            measure_key(&text, &new, Some(240.0))
        );
    }

    #[test]
    fn sync_text_block_preserves_layout_for_paint_only_style_changes() {
        let mut text_system = TextSystem::new();
        let text = Arc::<str>::from("hello color");
        let old = ResolvedTextStyle::default();
        let mut new = old.clone();
        new.color = Color::rgb(0x336699);

        let mut block = text_system.new_text_block(text.clone(), old, Some(240.0));
        let original_layout = block.layout.clone();
        let original_revision = block.revision;

        text_system.sync_text_block(&mut block, text, new.clone(), Some(240.0));

        assert!(Arc::ptr_eq(&original_layout, &block.layout));
        assert_eq!(block.revision.layout, original_revision.layout);
        assert_eq!(block.revision.style, original_revision.style + 1);
        assert_eq!(block.style, new);
    }

    #[test]
    fn subpixel_mask_falls_back_to_grayscale_alpha() {
        let bytes = [0_u8, 120, 240, 255, 30, 60, 90, 255];
        let alpha = subpixel_mask_to_alpha(&bytes);
        assert_eq!(alpha, vec![120, 60]);
    }

    #[test]
    fn nowrap_ellipsis_uses_requested_width_constraint() {
        let mut text_system = TextSystem::new();
        let text =
            Arc::<str>::from("This is a deliberately long single-line text run for ellipsis.");
        let style = ResolvedTextStyle {
            white_space: WhiteSpace::Nowrap,
            text_overflow: Some(TextOverflow::Ellipsis),
            ..ResolvedTextStyle::default()
        };

        let layout = text_system.measure(&text, &style, Some(140.0));
        assert!(layout.width <= 140.0);
    }
}
