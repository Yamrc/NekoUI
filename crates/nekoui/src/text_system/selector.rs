use std::sync::Arc;

use cosmic_text::{
    Align, Attrs, Family, Stretch as CosmicFontStretch, Style as CosmicFontStyle,
    Weight as CosmicFontWeight,
    fontdb::{FaceInfo, ID as CosmicFontId, Query},
};

use crate::SharedString;
use crate::style::{FontFamily, FontStyle, FontWeight, ResolvedTextStyle, TextAlign};

use super::{CLUSTER_FAMILY_INDEX_CACHE_LIMIT, FAMILY_CANDIDATE_CACHE_LIMIT, TextSystem};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct FamilyCandidateCacheKey {
    pub(super) family: FontFamily,
    pub(super) font_weight: FontWeight,
    pub(super) font_style: FontStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ClusterFamilyIndexCacheKey {
    pub(super) font_families: Arc<[FontFamily]>,
    pub(super) font_weight: FontWeight,
    pub(super) font_style: FontStyle,
    pub(super) cluster: SharedString,
}

impl TextSystem {
    pub(super) fn rich_text_spans(
        &mut self,
        text: &str,
        style: &ResolvedTextStyle,
    ) -> Vec<(std::ops::Range<usize>, Option<usize>)> {
        if text.is_empty() {
            return vec![(0..0, None)];
        }

        let mut spans = Vec::new();
        let mut current_start = 0usize;
        let mut current_family = None;

        for (start, grapheme) in
            unicode_segmentation::UnicodeSegmentation::grapheme_indices(text, true)
        {
            let end = start + grapheme.len();
            let family_index = self.family_index_for_cluster(grapheme, style);
            if start == 0 {
                current_start = 0;
                current_family = family_index;
                continue;
            }
            if family_index != current_family {
                spans.push((current_start..start, current_family));
                current_start = start;
                current_family = family_index;
            }
            if end == text.len() {
                spans.push((current_start..end, current_family));
            }
        }

        if spans.is_empty() {
            spans.push((0..text.len(), current_family));
        }
        spans
    }

    fn family_index_for_cluster(
        &mut self,
        cluster: &str,
        style: &ResolvedTextStyle,
    ) -> Option<usize> {
        let cache_key = ClusterFamilyIndexCacheKey {
            font_families: style.font_families.clone(),
            font_weight: style.font_weight,
            font_style: style.font_style,
            cluster: SharedString::from(cluster),
        };
        if let Some(cached) = self.cluster_family_index_cache.get(&cache_key) {
            return *cached;
        }

        if cluster.is_empty() {
            return None;
        }

        let selected = self.select_family_index_for_cluster(cluster, style);

        if self.cluster_family_index_cache.len() >= CLUSTER_FAMILY_INDEX_CACHE_LIMIT {
            self.cluster_family_index_cache.clear();
        }
        self.cluster_family_index_cache.insert(cache_key, selected);
        selected
    }

    fn select_family_index_for_cluster(
        &mut self,
        cluster: &str,
        style: &ResolvedTextStyle,
    ) -> Option<usize> {
        for (index, family) in style.font_families.iter().enumerate() {
            let candidates =
                self.candidate_font_ids_for_family(family, style.font_weight, style.font_style);
            for font_id in &*candidates {
                if font_supports_cluster(self.font_system.db(), *font_id, cluster) {
                    return Some(index);
                }
            }
        }

        None
    }

    fn candidate_font_ids_for_family(
        &mut self,
        family: &FontFamily,
        requested_weight: FontWeight,
        requested_style: FontStyle,
    ) -> Arc<[CosmicFontId]> {
        let cache_key = FamilyCandidateCacheKey {
            family: family.clone(),
            font_weight: requested_weight,
            font_style: requested_style,
        };
        if let Some(cached) = self.family_candidate_cache.get(&cache_key) {
            return cached.clone();
        }

        let candidates = collect_candidate_font_ids(
            self.font_system.db(),
            family,
            font_weight(requested_weight),
            font_style(requested_style),
        );
        let candidates: Arc<[CosmicFontId]> = Arc::from(candidates.into_boxed_slice());

        if self.family_candidate_cache.len() >= FAMILY_CANDIDATE_CACHE_LIMIT {
            self.family_candidate_cache.clear();
        }
        self.family_candidate_cache
            .insert(cache_key, candidates.clone());
        candidates
    }
}

pub(super) fn default_text_attrs(style: &ResolvedTextStyle) -> Attrs<'_> {
    let family = style
        .font_families
        .first()
        .expect("resolved text style must have at least one font family");
    text_attrs(style, family)
}

pub(super) fn text_attrs<'a>(style: &'a ResolvedTextStyle, family: &'a FontFamily) -> Attrs<'a> {
    let attrs = Attrs::new()
        .weight(font_weight(style.font_weight))
        .style(font_style(style.font_style));
    attrs.family(font_family(family))
}

pub(super) fn text_align(align: TextAlign) -> Align {
    match align {
        TextAlign::Start => Align::Left,
        TextAlign::Center => Align::Center,
        TextAlign::End => Align::Right,
    }
}

fn collect_candidate_font_ids(
    db: &cosmic_text::fontdb::Database,
    family: &FontFamily,
    target_weight: CosmicFontWeight,
    target_style: CosmicFontStyle,
) -> Vec<CosmicFontId> {
    match family {
        FontFamily::Named(name) => {
            let mut candidates = db
                .faces()
                .filter(|face| face_matches_named_family(face, name))
                .collect::<Vec<_>>();
            candidates.sort_by_key(|face| {
                face_sort_key(face, target_weight, CosmicFontStretch::Normal, target_style)
            });
            candidates.into_iter().map(|face| face.id).collect()
        }
        _ => {
            let query_family = font_family(family);
            let query = Query {
                families: std::slice::from_ref(&query_family),
                weight: target_weight,
                stretch: CosmicFontStretch::Normal,
                style: target_style,
            };
            db.query(&query).into_iter().collect()
        }
    }
}

fn face_matches_named_family(face: &FaceInfo, target: &str) -> bool {
    face.families
        .iter()
        .any(|(family, _)| family.eq_ignore_ascii_case(target))
}

fn face_sort_key(
    face: &FaceInfo,
    target_weight: CosmicFontWeight,
    target_stretch: CosmicFontStretch,
    target_style: CosmicFontStyle,
) -> (bool, u16, u16, u8, u16, u16) {
    (
        !face.post_script_name.contains("Emoji"),
        target_weight.0.abs_diff(face.weight.0),
        target_stretch
            .to_number()
            .abs_diff(face.stretch.to_number()),
        font_style_diff(target_style, face.style),
        face.weight.0,
        face.stretch.to_number(),
    )
}

fn font_supports_cluster(
    db: &cosmic_text::fontdb::Database,
    font_id: CosmicFontId,
    cluster: &str,
) -> bool {
    db.with_face_data(font_id, |font_data, face_index| {
        swash::FontRef::from_index(font_data, face_index as usize).is_some_and(|font| {
            let charmap = font.charmap();
            cluster
                .chars()
                .filter(|ch| !is_cluster_format_char(*ch))
                .all(|ch| charmap.map(ch) != 0)
        })
    })
    .unwrap_or(false)
}

fn is_cluster_format_char(ch: char) -> bool {
    ch.is_control() || matches!(ch, '\u{200c}' | '\u{200d}' | '\u{fe00}'..='\u{fe0f}')
}

fn font_style_diff(target: CosmicFontStyle, actual: cosmic_text::fontdb::Style) -> u8 {
    match (target, actual) {
        (cosmic_text::fontdb::Style::Normal, cosmic_text::fontdb::Style::Normal)
        | (cosmic_text::fontdb::Style::Italic, cosmic_text::fontdb::Style::Italic)
        | (cosmic_text::fontdb::Style::Oblique, cosmic_text::fontdb::Style::Oblique) => 0,
        (cosmic_text::fontdb::Style::Italic, cosmic_text::fontdb::Style::Oblique)
        | (cosmic_text::fontdb::Style::Oblique, cosmic_text::fontdb::Style::Italic) => 1,
        (cosmic_text::fontdb::Style::Normal, cosmic_text::fontdb::Style::Italic)
        | (cosmic_text::fontdb::Style::Normal, cosmic_text::fontdb::Style::Oblique)
        | (cosmic_text::fontdb::Style::Italic, cosmic_text::fontdb::Style::Normal)
        | (cosmic_text::fontdb::Style::Oblique, cosmic_text::fontdb::Style::Normal) => 2,
    }
}

fn font_weight(weight: FontWeight) -> CosmicFontWeight {
    match weight {
        FontWeight::Normal => CosmicFontWeight::NORMAL,
        FontWeight::Medium => CosmicFontWeight::MEDIUM,
        FontWeight::Semibold => CosmicFontWeight::SEMIBOLD,
        FontWeight::Bold => CosmicFontWeight::BOLD,
    }
}

fn font_style(style: FontStyle) -> CosmicFontStyle {
    match style {
        FontStyle::Normal => CosmicFontStyle::Normal,
        FontStyle::Italic => CosmicFontStyle::Italic,
    }
}

fn font_family(family: &FontFamily) -> Family<'_> {
    match family {
        FontFamily::Named(name) => Family::Name(name),
        FontFamily::Serif => Family::Serif,
        FontFamily::SansSerif => Family::SansSerif,
        FontFamily::Monospace => Family::Monospace,
        FontFamily::Cursive => Family::Cursive,
        FontFamily::Fantasy => Family::Fantasy,
        FontFamily::SystemUi => Family::SansSerif,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::style::{FontFamily, ResolvedTextStyle};

    use super::{TextSystem, font_supports_cluster};

    #[test]
    fn user_family_stack_prefers_later_family_before_platform_fallback() {
        let mut text_system = TextSystem::new();
        let caskaydia = FontFamily::from("CaskaydiaCove Nerd Font");
        let noto = FontFamily::from("Noto Sans SC");
        let caskaydia_ids = text_system.candidate_font_ids_for_family(
            &caskaydia,
            Default::default(),
            Default::default(),
        );
        let noto_ids = text_system.candidate_font_ids_for_family(
            &noto,
            Default::default(),
            Default::default(),
        );

        if caskaydia_ids.is_empty() || noto_ids.is_empty() {
            return;
        }

        let db = text_system.font_system.db();
        let caskaydia_supports_ascii = caskaydia_ids
            .iter()
            .any(|font_id| font_supports_cluster(db, *font_id, "A"));
        let caskaydia_supports_cjk = caskaydia_ids
            .iter()
            .any(|font_id| font_supports_cluster(db, *font_id, "我"));
        let noto_supports_cjk = noto_ids
            .iter()
            .any(|font_id| font_supports_cluster(db, *font_id, "我"));

        if !caskaydia_supports_ascii || caskaydia_supports_cjk || !noto_supports_cjk {
            return;
        }

        let style = ResolvedTextStyle {
            font_families: Arc::from([caskaydia, noto]),
            ..ResolvedTextStyle::default()
        };
        let text = "A我";
        let cjk_start = text.find('我').expect("test text must contain CJK sample");
        let spans = text_system.rich_text_spans(text, &style);
        let family_index = spans
            .iter()
            .find(|(range, _)| range.contains(&cjk_start))
            .and_then(|(_, family_index)| *family_index);

        assert_eq!(family_index, Some(1));

        let layout = text_system.measure(&text.into(), &style, None);
        let cjk_glyph = layout
            .runs
            .iter()
            .flat_map(|run| run.glyphs.iter())
            .find(|glyph| glyph.start == cjk_start)
            .expect("CJK sample should produce a glyph");
        assert!(noto_ids.contains(&cjk_glyph.font_id));
    }
}
