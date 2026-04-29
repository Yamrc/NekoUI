use std::sync::Arc;

use cosmic_text::{CacheKey, SwashContent};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::platform::wgpu::atlas::{AtlasEntry, GlyphAtlasKind};
use crate::scene::{CompiledScene, MaterialClass, Primitive, SceneNodeId};
use crate::text_system::TextSystem;

use super::{
    RenderSystem,
    submit::push_gpu_batch,
    types::{
        ActiveBatch, ClipReference, ClipSlotInstance, ClipStack, ColorTextInstance, GpuBatch,
        LogicalBatchCursor, PipelineKey, PreparedFrame, PreparedFrameKey, RectInstance,
        SceneWalkState, TextInstance, TextPrimitiveParams, TextureBindingKey,
    },
};

struct PrepareContext<'a> {
    text_system: &'a mut TextSystem,
    scale_factor: f32,
    clip_cache: FxHashMap<ClipStackKey, ClipReference>,
}

impl RenderSystem {
    pub(super) fn prepare_frame(
        &mut self,
        state: &mut super::types::WindowRenderState,
        scene: &CompiledScene,
        text_system: &mut TextSystem,
        scale_factor: f32,
    ) {
        self.mono_atlas.begin_frame();
        self.color_atlas.begin_frame();

        let current_key = self.prepared_frame_key(scene, scale_factor);
        if state
            .prepared_frame
            .as_ref()
            .is_some_and(|prepared| prepared.key == current_key)
        {
            return;
        }

        self.rect_instances.clear();
        self.mono_text_instances.clear();
        self.color_text_instances.clear();
        self.clip_slots.clear();
        self.gpu_batches.clear();
        self.collect_instances(scene, text_system, scale_factor);
        let final_key = self.prepared_frame_key(scene, scale_factor);
        state.prepared_frame = Some(PreparedFrame {
            key: final_key,
            rect_instances: Arc::new(std::mem::take(&mut self.rect_instances)),
            mono_text_instances: Arc::new(std::mem::take(&mut self.mono_text_instances)),
            color_text_instances: Arc::new(std::mem::take(&mut self.color_text_instances)),
            clip_slots: Arc::new(std::mem::take(&mut self.clip_slots)),
            gpu_batches: Arc::new(std::mem::take(&mut self.gpu_batches)),
            uploaded_buffer_epoch: 0,
        });
    }

    pub(super) fn prepared_frame_key(
        &self,
        scene: &CompiledScene,
        scale_factor: f32,
    ) -> PreparedFrameKey {
        PreparedFrameKey {
            scene_nodes: scene.scene_nodes.clone(),
            primitives: scene.primitives.clone(),
            logical_batches: scene.logical_batches.clone(),
            scale_factor_bits: scale_factor.to_bits(),
            mono_atlas_generation: self.mono_atlas.generation(),
            color_atlas_generation: self.color_atlas.generation(),
        }
    }

    fn collect_instances(
        &mut self,
        scene: &CompiledScene,
        text_system: &mut TextSystem,
        scale_factor: f32,
    ) {
        let scale_factor = scale_factor.max(f32::MIN_POSITIVE);
        if scene.scene_nodes.is_empty()
            || scene.primitives.is_empty()
            || scene.logical_batches.is_empty()
        {
            return;
        }

        let mut prepare_context = PrepareContext {
            text_system,
            scale_factor,
            clip_cache: FxHashMap::default(),
        };
        let mut batch_cursor = LogicalBatchCursor::new(&scene.logical_batches);
        self.collect_node_instances(
            scene,
            SceneNodeId(0),
            SceneWalkState {
                offset: [0.0, 0.0],
                opacity: 1.0,
                clip: ClipStack::default(),
            },
            &mut batch_cursor,
            &mut prepare_context,
        );
    }

    pub(super) fn ensure_glyph_entry(
        &mut self,
        text_system: &mut TextSystem,
        cache_key: CacheKey,
    ) -> Option<(GlyphAtlasKind, AtlasEntry)> {
        if let Some(entry) = self.mono_atlas.get(&cache_key) {
            return Some((GlyphAtlasKind::Mono, entry));
        }
        if let Some(entry) = self.color_atlas.get(&cache_key) {
            return Some((GlyphAtlasKind::Color, entry));
        }

        let image = self
            .swash_cache
            .get_image(text_system.font_system_mut(), cache_key)
            .as_ref()?
            .clone();

        match image.content {
            SwashContent::Color => self
                .color_atlas
                .upload_color(&self.context.device, &self.context.queue, cache_key, &image)
                .map(|entry| (GlyphAtlasKind::Color, entry)),
            SwashContent::Mask | SwashContent::SubpixelMask => self
                .mono_atlas
                .upload_mask(&self.context.device, &self.context.queue, cache_key, &image)
                .map(|entry| (GlyphAtlasKind::Mono, entry)),
        }
    }

    fn instance_count_for(&self, pipeline_key: PipelineKey) -> u32 {
        match pipeline_key {
            PipelineKey::Rect => self.rect_instances.len() as u32,
            PipelineKey::MonoText => self.mono_text_instances.len() as u32,
            PipelineKey::ColorText => self.color_text_instances.len() as u32,
        }
    }

    fn collect_node_instances(
        &mut self,
        scene: &CompiledScene,
        node_id: SceneNodeId,
        parent_state: SceneWalkState,
        batch_cursor: &mut LogicalBatchCursor<'_>,
        prepare_context: &mut PrepareContext<'_>,
    ) {
        let node = &scene.scene_nodes[node_id.0 as usize];
        let current_offset = [
            parent_state.offset[0] + node.transform.tx,
            parent_state.offset[1] + node.transform.ty,
        ];
        let current_opacity = parent_state.opacity * node.opacity;
        let local_clip = node
            .clip
            .shape
            .map(|shape| shape.translate(current_offset[0], current_offset[1]));
        let current_state = SceneWalkState {
            offset: current_offset,
            opacity: current_opacity,
            clip: combine_clip_stack(parent_state.clip.clone(), local_clip),
        };

        for primitive_index in node.primitive_range.as_range() {
            let batch = batch_cursor.batch_for_primitive(primitive_index as u32);
            match &scene.primitives[primitive_index] {
                Primitive::Rect(rect_primitive) => {
                    debug_assert_eq!(batch.material_class, MaterialClass::Rect);
                    let start = self.rect_instances.len() as u32;
                    let rect_bounds = crate::scene::LayoutBox {
                        x: rect_primitive.bounds.x + current_state.offset[0],
                        y: rect_primitive.bounds.y + current_state.offset[1],
                        width: rect_primitive.bounds.width,
                        height: rect_primitive.bounds.height,
                    };
                    if !intersects_clip(rect_bounds, &current_state.clip) {
                        continue;
                    }
                    let clip_reference = self.prepare_clip_reference(
                        current_state.clip.clone(),
                        prepare_context.scale_factor,
                        &mut prepare_context.clip_cache,
                    );
                    self.rect_instances.push(RectInstance::from_primitive(
                        rect_primitive,
                        rect_bounds,
                        clip_reference,
                        prepare_context.scale_factor,
                        current_state.opacity,
                    ));
                    self.push_gpu_batch(
                        PipelineKey::Rect,
                        TextureBindingKey::None,
                        current_state.clip.clone(),
                        batch.effect_class,
                        start..self.rect_instances.len() as u32,
                    );
                }
                Primitive::Text {
                    bounds,
                    layout,
                    color,
                } => {
                    debug_assert_eq!(batch.material_class, MaterialClass::Text);
                    self.collect_text_primitive_instances(
                        TextPrimitiveParams {
                            bounds,
                            layout,
                            color,
                            scene_state: current_state.clone(),
                            batch,
                        },
                        prepare_context,
                    );
                }
            }
        }

        let mut child = node.first_child;
        while let Some(child_id) = child {
            self.collect_node_instances(
                scene,
                child_id,
                current_state.clone(),
                batch_cursor,
                prepare_context,
            );
            child = scene.scene_nodes[child_id.0 as usize].next_sibling;
        }
    }

    fn collect_text_primitive_instances(
        &mut self,
        params: TextPrimitiveParams<'_>,
        prepare_context: &mut PrepareContext<'_>,
    ) {
        let mut active_batch = None;
        let text_bounds = crate::scene::LayoutBox {
            x: params.bounds.x + params.scene_state.offset[0],
            y: params.bounds.y + params.scene_state.offset[1],
            width: params.bounds.width,
            height: params.bounds.height,
        };
        if !intersects_clip(text_bounds, &params.scene_state.clip) {
            return;
        }
        let scaled_clip = params
            .scene_state
            .clip
            .scissor_bounds()
            .map(|clip| scale_layout_box(clip, prepare_context.scale_factor));
        let clip_reference = self.prepare_clip_reference(
            params.scene_state.clip.clone(),
            prepare_context.scale_factor,
            &mut prepare_context.clip_cache,
        );

        for run in &*params.layout.runs {
            for glyph in &run.glyphs {
                if !logical_glyph_may_intersect_clip(
                    glyph,
                    text_bounds,
                    params.layout.height,
                    &params.scene_state.clip,
                ) {
                    continue;
                }

                let glyph_color = glyph
                    .color_opt
                    .map(super::cosmic_to_style_color)
                    .unwrap_or(*params.color);
                let physical = glyph.physical(
                    (
                        text_bounds.x * prepare_context.scale_factor,
                        (text_bounds.y + run.baseline) * prepare_context.scale_factor,
                    ),
                    prepare_context.scale_factor,
                );
                let Some((atlas_kind, entry)) =
                    self.ensure_glyph_entry(prepare_context.text_system, physical.cache_key)
                else {
                    continue;
                };

                match atlas_kind {
                    GlyphAtlasKind::Mono => {
                        let rect = crate::scene::LayoutBox {
                            x: (physical.x + entry.placement_left) as f32,
                            y: (physical.y - entry.placement_top) as f32,
                            width: entry.width as f32,
                            height: entry.height as f32,
                        };
                        if !intersects_clip_bounds(rect, scaled_clip) {
                            continue;
                        }
                        let uv = crate::scene::LayoutBox {
                            x: entry.uv_rect[0],
                            y: entry.uv_rect[1],
                            width: entry.uv_rect[2],
                            height: entry.uv_rect[3],
                        };
                        self.start_or_switch_batch(
                            &mut active_batch,
                            PipelineKey::MonoText,
                            TextureBindingKey::MonoGlyphAtlas(entry.page_id),
                            params.scene_state.clip.clone(),
                            params.batch.effect_class,
                        );
                        self.mono_text_instances.push(TextInstance {
                            rect: [rect.x, rect.y, rect.width, rect.height],
                            uv_rect: [uv.x, uv.y, uv.width, uv.height],
                            color: [
                                glyph_color.r,
                                glyph_color.g,
                                glyph_color.b,
                                glyph_color.a * params.scene_state.opacity,
                            ],
                            clip_reference: clip_reference.into_raw(),
                        });
                    }
                    GlyphAtlasKind::Color => {
                        let cache_key = color_glyph_cache_key(glyph, prepare_context.scale_factor);
                        let Some((_, entry)) =
                            self.ensure_glyph_entry(prepare_context.text_system, cache_key)
                        else {
                            continue;
                        };
                        let color_origin = color_glyph_origin(
                            glyph,
                            text_bounds,
                            run.baseline,
                            prepare_context.scale_factor,
                        );
                        let rect = crate::scene::LayoutBox {
                            x: color_origin[0] + entry.placement_left as f32,
                            y: color_origin[1] - entry.placement_top as f32,
                            width: entry.width as f32,
                            height: entry.height as f32,
                        };
                        if !intersects_clip_bounds(rect, scaled_clip) {
                            continue;
                        }
                        let uv = crate::scene::LayoutBox {
                            x: entry.uv_rect[0],
                            y: entry.uv_rect[1],
                            width: entry.uv_rect[2],
                            height: entry.uv_rect[3],
                        };
                        let rect = [
                            color_origin[0] + entry.placement_left as f32,
                            color_origin[1] - entry.placement_top as f32,
                            entry.width as f32,
                            entry.height as f32,
                        ];
                        self.start_or_switch_batch(
                            &mut active_batch,
                            PipelineKey::ColorText,
                            TextureBindingKey::ColorGlyphAtlas(entry.page_id),
                            params.scene_state.clip.clone(),
                            params.batch.effect_class,
                        );
                        self.color_text_instances.push(ColorTextInstance {
                            rect,
                            uv_rect: [uv.x, uv.y, uv.width, uv.height],
                            alpha: [glyph_color.a * params.scene_state.opacity, 0.0, 0.0, 0.0],
                            clip_reference: clip_reference.into_raw(),
                        });
                    }
                }
            }
        }

        self.finish_active_batch(
            &mut active_batch,
            params.scene_state.clip.clone(),
            params.batch.effect_class,
        );
    }

    fn start_or_switch_batch(
        &mut self,
        active_batch: &mut Option<ActiveBatch>,
        pipeline_key: super::types::PipelineKey,
        texture_binding: super::types::TextureBindingKey,
        clip_stack: ClipStack,
        effect_class: crate::scene::EffectClass,
    ) {
        let next_batch = ActiveBatch {
            pipeline_key,
            texture_binding,
            start: self.instance_count_for(pipeline_key),
        };

        if matches!(
            active_batch,
            Some(active)
                if active.pipeline_key == next_batch.pipeline_key
                    && active.texture_binding == next_batch.texture_binding
        ) {
            return;
        }

        self.finish_active_batch(active_batch, clip_stack, effect_class);
        *active_batch = Some(next_batch);
    }

    fn finish_active_batch(
        &mut self,
        active_batch: &mut Option<ActiveBatch>,
        clip_stack: ClipStack,
        effect_class: crate::scene::EffectClass,
    ) {
        let Some(active_batch) = active_batch.take() else {
            return;
        };

        self.push_gpu_batch(
            active_batch.pipeline_key,
            active_batch.texture_binding,
            clip_stack,
            effect_class,
            active_batch.start..self.instance_count_for(active_batch.pipeline_key),
        );
    }

    fn push_gpu_batch(
        &mut self,
        pipeline_key: PipelineKey,
        texture_binding: TextureBindingKey,
        clip_stack: ClipStack,
        effect_class: crate::scene::EffectClass,
        instance_range: std::ops::Range<u32>,
    ) {
        push_gpu_batch(
            &mut self.gpu_batches,
            GpuBatch {
                pipeline_key,
                texture_binding,
                clip_stack,
                effect_class,
                instance_range,
            },
        );
    }

    fn prepare_clip_reference(
        &mut self,
        clip_stack: ClipStack,
        scale_factor: f32,
        clip_cache: &mut FxHashMap<ClipStackKey, ClipReference>,
    ) -> ClipReference {
        if clip_stack.is_empty() {
            return ClipReference::NONE;
        }

        let key = ClipStackKey::from_stack(&clip_stack);
        if let Some(reference) = clip_cache.get(&key) {
            return *reference;
        }

        let offset = self.clip_slots.len() as u32;
        for clip_shape in clip_stack.iter() {
            self.clip_slots
                .push(ClipSlotInstance::from_shape(clip_shape, scale_factor));
        }
        let reference = ClipReference {
            offset,
            len: (self.clip_slots.len() as u32).saturating_sub(offset),
        };
        clip_cache.insert(key, reference);
        reference
    }
}

fn color_glyph_cache_key(glyph: &cosmic_text::LayoutGlyph, scale_factor: f32) -> CacheKey {
    let (cache_key, _, _) = CacheKey::new(
        glyph.font_id,
        glyph.glyph_id,
        glyph.font_size * scale_factor,
        (0.0, 0.0),
        glyph.font_weight,
        glyph.cache_key_flags,
    );
    cache_key
}

fn color_glyph_origin(
    glyph: &cosmic_text::LayoutGlyph,
    text_bounds: crate::scene::LayoutBox,
    baseline: f32,
    scale_factor: f32,
) -> [f32; 2] {
    let x_offset = glyph.font_size * glyph.x_offset;
    let y_offset = glyph.font_size * glyph.y_offset;
    let x = ((text_bounds.x + glyph.x + x_offset) * scale_factor).floor();
    let y = ((text_bounds.y + baseline + glyph.y - y_offset) * scale_factor).floor();
    [x, y]
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ClipStackKey {
    slots: SmallVec<[PackedClipShapeKey; 4]>,
}

impl ClipStackKey {
    fn from_stack(clip_stack: &ClipStack) -> Self {
        Self {
            slots: clip_stack
                .iter()
                .map(PackedClipShapeKey::from_shape)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PackedClipShapeKey {
    kind: u8,
    bounds: [u32; 4],
    radii: [u32; 4],
}

impl PackedClipShapeKey {
    fn from_shape(clip_shape: crate::scene::ClipShape) -> Self {
        match clip_shape {
            crate::scene::ClipShape::Rect(bounds) => Self {
                kind: 0,
                bounds: [
                    bounds.x.to_bits(),
                    bounds.y.to_bits(),
                    bounds.width.to_bits(),
                    bounds.height.to_bits(),
                ],
                radii: [0; 4],
            },
            crate::scene::ClipShape::RoundedRect {
                bounds,
                corner_radii,
            } => Self {
                kind: 1,
                bounds: [
                    bounds.x.to_bits(),
                    bounds.y.to_bits(),
                    bounds.width.to_bits(),
                    bounds.height.to_bits(),
                ],
                radii: [
                    corner_radii.top_left.to_bits(),
                    corner_radii.top_right.to_bits(),
                    corner_radii.bottom_right.to_bits(),
                    corner_radii.bottom_left.to_bits(),
                ],
            },
        }
    }
}

fn intersects_clip(rect: crate::scene::LayoutBox, clip: &ClipStack) -> bool {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return false;
    }

    if clip.is_empty() {
        return true;
    }

    clip.scissor_bounds()
        .is_some_and(|clip_bounds| intersect_rect(rect, clip_bounds).is_some())
}

fn intersects_clip_bounds(
    rect: crate::scene::LayoutBox,
    clip_bounds: Option<crate::scene::LayoutBox>,
) -> bool {
    rect.width > 0.0
        && rect.height > 0.0
        && clip_bounds.is_none_or(|clip_bounds| intersect_rect(rect, clip_bounds).is_some())
}

fn logical_glyph_may_intersect_clip(
    glyph: &cosmic_text::LayoutGlyph,
    text_bounds: crate::scene::LayoutBox,
    layout_height: f32,
    clip: &ClipStack,
) -> bool {
    let x_offset = glyph.font_size * glyph.x_offset;
    let left = text_bounds.x + glyph.x + x_offset.min(0.0);
    let width = (glyph.w + x_offset.abs()).max(glyph.font_size * 0.5);
    intersects_clip(
        crate::scene::LayoutBox {
            x: left,
            y: text_bounds.y,
            width,
            height: layout_height.max(glyph.font_size),
        },
        clip,
    )
}

fn scale_layout_box(rect: crate::scene::LayoutBox, scale_factor: f32) -> crate::scene::LayoutBox {
    crate::scene::LayoutBox {
        x: rect.x * scale_factor,
        y: rect.y * scale_factor,
        width: rect.width * scale_factor,
        height: rect.height * scale_factor,
    }
}

fn combine_clip_stack(
    clip_stack: ClipStack,
    local_clip: Option<crate::scene::ClipShape>,
) -> ClipStack {
    let Some(local_clip) = local_clip else {
        return clip_stack;
    };

    clip_stack.push(local_clip)
}

pub(super) fn intersect_rect(
    a: crate::scene::LayoutBox,
    b: crate::scene::LayoutBox,
) -> Option<crate::scene::LayoutBox> {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);

    if right <= left || bottom <= top {
        return None;
    }

    Some(crate::scene::LayoutBox {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}
