mod cache;
mod selector;
mod types;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cosmic_text::{Align, Attrs, Buffer, FontSystem, Metrics, Shaping, Wrap};
use unicode_segmentation::UnicodeSegmentation;

use crate::SharedString;
use crate::style::{Absolute, Definite, ResolvedTextStyle, TextOverflow, WhiteSpace};

use self::cache::AdaptiveLruCache;
use self::selector::{
    ClusterFamilyIndexCacheKey, FamilyCandidateCacheKey, default_text_attrs, text_align, text_attrs,
};
pub use self::types::{SharedTextLayout, TextCacheStats, TextLayout, TextMeasureKey, TextRun};

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
        let layout = self.shape_and_layout(text, style, width);
        let shared = Arc::new(layout);
        self.measure_cache.insert(key, shared.clone());
        shared
    }

    fn shape_and_layout(
        &mut self,
        text: &SharedString,
        style: &ResolvedTextStyle,
        width: Option<f32>,
    ) -> TextLayout {
        let font_size = resolve_absolute(style.font_size);
        let line_height = style
            .line_height
            .map(|line_height| resolve_definite(line_height, font_size))
            .unwrap_or(font_size * 1.2);
        let metrics = Metrics::new(font_size, line_height);
        let attrs = default_text_attrs(style);
        let alignment = Some(text_align(style.text_align));
        let wrap = match style.white_space {
            WhiteSpace::Normal => Wrap::WordOrGlyph,
            WhiteSpace::Nowrap => Wrap::None,
        };
        let effective_width = match style.white_space {
            WhiteSpace::Normal => width,
            WhiteSpace::Nowrap => None,
        };

        let mut buffer = self.build_buffer(BuildBufferRequest {
            text,
            metrics,
            style,
            attrs: &attrs,
            width: effective_width,
            alignment,
            wrap,
        });

        if matches!(style.white_space, WhiteSpace::Nowrap)
            && matches!(style.text_overflow, Some(TextOverflow::Ellipsis))
            && let Some(max_width) = width
            && current_layout_width(&buffer) > max_width
        {
            let truncated = self.truncate_text_to_width(text, metrics, &attrs, style, max_width);
            buffer = self.build_buffer(BuildBufferRequest {
                text: &truncated,
                metrics,
                style,
                attrs: &attrs,
                width: Some(max_width),
                alignment,
                wrap: Wrap::None,
            });
        }

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

    fn build_buffer(&mut self, request: BuildBufferRequest<'_>) -> Buffer {
        let mut buffer = Buffer::new(&mut self.font_system, request.metrics);
        buffer.set_size(&mut self.font_system, request.width, None);
        buffer.set_wrap(&mut self.font_system, request.wrap);
        let spans = self.rich_text_spans(request.text, request.style);
        buffer.set_rich_text(
            &mut self.font_system,
            spans.iter().map(|(range, family_index)| {
                (
                    &request.text[range.clone()],
                    family_index
                        .and_then(|index| request.style.font_families.get(index))
                        .map_or_else(
                            || request.attrs.clone(),
                            |family| text_attrs(request.style, family),
                        ),
                )
            }),
            request.attrs,
            Shaping::Advanced,
            request.alignment,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);
        buffer
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
        let buffer = self.build_buffer(BuildBufferRequest {
            text,
            metrics,
            style,
            attrs,
            width: None,
            alignment: Some(Align::Left),
            wrap: Wrap::None,
        });
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

    pub(crate) fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
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
        style_hash: hash_text_style(style),
        width_bits: width.map(f32::to_bits),
    }
}

fn hash_text_style(style: &ResolvedTextStyle) -> u64 {
    let mut hasher = DefaultHasher::new();
    style.font_families.hash(&mut hasher);
    hash_absolute(style.font_size, &mut hasher);
    hash_option_definite(style.line_height, &mut hasher);
    style.font_weight.hash(&mut hasher);
    style.font_style.hash(&mut hasher);
    style.text_align.hash(&mut hasher);
    style.white_space.hash(&mut hasher);
    style.text_overflow.hash(&mut hasher);
    style.color.r.to_bits().hash(&mut hasher);
    style.color.g.to_bits().hash(&mut hasher);
    style.color.b.to_bits().hash(&mut hasher);
    style.color.a.to_bits().hash(&mut hasher);
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

struct BuildBufferRequest<'a> {
    text: &'a str,
    metrics: Metrics,
    style: &'a ResolvedTextStyle,
    attrs: &'a Attrs<'a>,
    width: Option<f32>,
    alignment: Option<Align>,
    wrap: Wrap,
}

fn hash_value(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
