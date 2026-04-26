use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};

use crate::scene::{ClipShape, EffectClass, LayoutBox, LogicalBatch, RectFill, RectPrimitive};
use crate::style::Color;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct ViewUniform {
    pub(super) viewport: [f32; 2],
    pub(super) _pad: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct RectInstance {
    pub(super) rect: [f32; 4],
    pub(super) fill_start_color: [f32; 4],
    pub(super) fill_end_color: [f32; 4],
    pub(super) fill_meta: [f32; 4],
    pub(super) corner_radii: [f32; 4],
    pub(super) border_widths: [f32; 4],
    pub(super) border_color: [f32; 4],
    pub(super) clip_rect_0: [f32; 4],
    pub(super) clip_corner_radii_0: [f32; 4],
    pub(super) clip_rect_1: [f32; 4],
    pub(super) clip_corner_radii_1: [f32; 4],
}

impl RectInstance {
    pub(super) fn from_primitive(
        primitive: &RectPrimitive,
        rect: LayoutBox,
        clip_stack: ClipStack,
        scale_factor: f32,
        opacity: f32,
    ) -> Self {
        let (fill_start_color, fill_end_color, fill_meta) = pack_rect_fill(primitive.fill, opacity);
        let (clip_rect_0, clip_corner_radii_0, clip_rect_1, clip_corner_radii_1) =
            pack_clip_stack(clip_stack, scale_factor);
        Self {
            rect: scale_layout_box(rect, scale_factor),
            fill_start_color,
            fill_end_color,
            fill_meta,
            corner_radii: scale_corners(primitive.corner_radii, scale_factor),
            border_widths: scale_edges(primitive.border_widths, scale_factor),
            border_color: primitive
                .border_color
                .map(|border_color| pack_color(border_color, opacity))
                .unwrap_or([0.0; 4]),
            clip_rect_0,
            clip_corner_radii_0,
            clip_rect_1,
            clip_corner_radii_1,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct TextInstance {
    pub(super) rect: [f32; 4],
    pub(super) uv_rect: [f32; 4],
    pub(super) color: [f32; 4],
    pub(super) clip_rect_0: [f32; 4],
    pub(super) clip_corner_radii_0: [f32; 4],
    pub(super) clip_rect_1: [f32; 4],
    pub(super) clip_corner_radii_1: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct ColorTextInstance {
    pub(super) rect: [f32; 4],
    pub(super) uv_rect: [f32; 4],
    pub(super) alpha: [f32; 4],
    pub(super) clip_rect_0: [f32; 4],
    pub(super) clip_corner_radii_0: [f32; 4],
    pub(super) clip_rect_1: [f32; 4],
    pub(super) clip_corner_radii_1: [f32; 4],
}

fn pack_rect_fill(fill: RectFill, opacity: f32) -> ([f32; 4], [f32; 4], [f32; 4]) {
    match fill {
        RectFill::Solid(color) => {
            let packed = pack_color(color, opacity);
            (packed, packed, [0.0, 0.0, 0.0, 0.0])
        }
        RectFill::LinearGradient(gradient) => (
            pack_color(gradient.start_color, opacity),
            pack_color(gradient.end_color, opacity),
            [1.0, gradient.angle_radians, 0.0, 0.0],
        ),
    }
}

fn pack_color(color: Color, opacity: f32) -> [f32; 4] {
    [color.r, color.g, color.b, color.a * opacity]
}

pub(super) fn pack_clip_stack(
    clip_stack: ClipStack,
    scale_factor: f32,
) -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
    fn pack_slot(clip_shape: Option<ClipShape>, scale_factor: f32) -> ([f32; 4], [f32; 4]) {
        let Some(clip_shape) = clip_shape else {
            return ([0.0; 4], [0.0; 4]);
        };

        match clip_shape {
            ClipShape::Rect(bounds) => (scale_layout_box(bounds, scale_factor), [0.0; 4]),
            ClipShape::RoundedRect {
                bounds,
                corner_radii,
            } => (
                scale_layout_box(bounds, scale_factor),
                scale_corners(corner_radii, scale_factor),
            ),
        }
    }

    let (clip_rect_0, clip_corner_radii_0) = pack_slot(clip_stack.first, scale_factor);
    let (clip_rect_1, clip_corner_radii_1) = pack_slot(clip_stack.second, scale_factor);
    (
        clip_rect_0,
        clip_corner_radii_0,
        clip_rect_1,
        clip_corner_radii_1,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct ClipStack {
    pub(super) first: Option<ClipShape>,
    pub(super) second: Option<ClipShape>,
}

impl ClipStack {
    pub(super) fn push(mut self, clip_shape: ClipShape) -> Self {
        if self.first == Some(clip_shape) || self.second == Some(clip_shape) {
            return self;
        }

        if self.first.is_none() {
            self.first = Some(clip_shape);
            return self;
        }

        if self.second.is_none() {
            self.second = Some(clip_shape);
            return self;
        }

        let collapsed = self
            .scissor_bounds()
            .and_then(|bounds| intersect_layout_box(bounds, clip_shape.bounds()))
            .map(ClipShape::Rect)
            .unwrap_or(ClipShape::Rect(LayoutBox {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            }));
        Self {
            first: Some(collapsed),
            second: None,
        }
    }

    pub(super) fn scissor_bounds(self) -> Option<LayoutBox> {
        match (self.first, self.second) {
            (Some(first), Some(second)) => intersect_layout_box(first.bounds(), second.bounds()),
            (Some(first), None) => Some(first.bounds()),
            (None, Some(second)) => Some(second.bounds()),
            (None, None) => None,
        }
    }

    pub(super) fn is_empty(self) -> bool {
        self.first.is_none() && self.second.is_none()
    }
}

fn intersect_layout_box(a: LayoutBox, b: LayoutBox) -> Option<LayoutBox> {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);

    if right <= left || bottom <= top {
        return None;
    }

    Some(LayoutBox {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

fn scale_layout_box(rect: LayoutBox, scale_factor: f32) -> [f32; 4] {
    [
        rect.x * scale_factor,
        rect.y * scale_factor,
        rect.width * scale_factor,
        rect.height * scale_factor,
    ]
}

fn scale_corners(corners: crate::style::CornerRadii, scale_factor: f32) -> [f32; 4] {
    [
        corners.top_left * scale_factor,
        corners.top_right * scale_factor,
        corners.bottom_right * scale_factor,
        corners.bottom_left * scale_factor,
    ]
}

fn scale_edges(edges: crate::style::EdgeWidths, scale_factor: f32) -> [f32; 4] {
    [
        edges.top * scale_factor,
        edges.right * scale_factor,
        edges.bottom * scale_factor,
        edges.left * scale_factor,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreparedFrameKey {
    pub(super) scene_arc_ptr: usize,
    pub(super) primitives_arc_ptr: usize,
    pub(super) logical_batches_arc_ptr: usize,
    pub(super) scale_factor_bits: u32,
    pub(super) mono_atlas_generation: u64,
    pub(super) color_atlas_generation: u64,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedFrame {
    pub(super) key: PreparedFrameKey,
    pub(super) rect_instances: Arc<Vec<RectInstance>>,
    pub(super) mono_text_instances: Arc<Vec<TextInstance>>,
    pub(super) color_text_instances: Arc<Vec<ColorTextInstance>>,
    pub(super) gpu_batches: Arc<Vec<GpuBatch>>,
    pub(super) uploaded_buffer_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PipelineKey {
    Rect,
    MonoText,
    ColorText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TextureBindingKey {
    None,
    MonoGlyphAtlas(u32),
    ColorGlyphAtlas(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ActiveBatch {
    pub(super) pipeline_key: PipelineKey,
    pub(super) texture_binding: TextureBindingKey,
    pub(super) start: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GpuBatch {
    pub(super) pipeline_key: PipelineKey,
    pub(super) texture_binding: TextureBindingKey,
    pub(super) clip_stack: ClipStack,
    pub(super) effect_class: EffectClass,
    pub(super) instance_range: Range<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BatchClipPolicy {
    None,
    Bounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BatchEffectPolicy {
    None,
    Opacity,
}

impl From<EffectClass> for BatchEffectPolicy {
    fn from(value: EffectClass) -> Self {
        match value {
            EffectClass::None => Self::None,
            EffectClass::Opacity => Self::Opacity,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EffectRenderPolicy {
    Direct,
    InlineOpacity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BatchSubmitState {
    pub(super) pipeline_key: PipelineKey,
    pub(super) texture_binding: TextureBindingKey,
    pub(super) clip_policy: BatchClipPolicy,
    pub(super) clip_stack: ClipStack,
    pub(super) effect_policy: BatchEffectPolicy,
    pub(super) effect_render_policy: EffectRenderPolicy,
}

impl From<&GpuBatch> for BatchSubmitState {
    fn from(batch: &GpuBatch) -> Self {
        let effect_policy: BatchEffectPolicy = batch.effect_class.into();
        Self {
            pipeline_key: batch.pipeline_key,
            texture_binding: batch.texture_binding,
            clip_policy: if batch.clip_stack.is_empty() {
                BatchClipPolicy::None
            } else {
                BatchClipPolicy::Bounds
            },
            clip_stack: batch.clip_stack,
            effect_policy,
            effect_render_policy: super::submit::effect_render_policy(effect_policy),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScissorRect {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

#[derive(Default)]
pub(super) struct GpuBatchBuilder {
    pub(super) batches: Vec<GpuBatch>,
}

impl GpuBatchBuilder {
    pub(super) fn push(&mut self, batch: GpuBatch) {
        if batch.instance_range.is_empty() {
            return;
        }

        if let Some(previous) = self.batches.last_mut()
            && super::submit::can_merge_gpu_batches(previous, &batch)
        {
            previous.instance_range.end = batch.instance_range.end;
            return;
        }

        self.batches.push(batch);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SceneWalkState {
    pub(super) offset: [f32; 2],
    pub(super) opacity: f32,
    pub(super) clip: ClipStack,
}

pub(super) struct TextPrimitiveParams<'a> {
    pub(super) bounds: &'a crate::scene::LayoutBox,
    pub(super) layout: &'a std::sync::Arc<crate::text_system::TextLayout>,
    pub(super) color: &'a Color,
    pub(super) scene_state: SceneWalkState,
    pub(super) batch: &'a LogicalBatch,
}

pub(super) struct LogicalBatchCursor<'a> {
    batches: &'a [LogicalBatch],
    index: usize,
}

impl<'a> LogicalBatchCursor<'a> {
    pub(super) fn new(batches: &'a [LogicalBatch]) -> Self {
        Self { batches, index: 0 }
    }

    pub(super) fn batch_for_primitive(&mut self, primitive_index: u32) -> &'a LogicalBatch {
        while self.index + 1 < self.batches.len()
            && primitive_index >= self.batches[self.index].primitive_range.end
        {
            self.index += 1;
        }

        let batch = &self.batches[self.index];
        debug_assert!(batch.primitive_range.start <= primitive_index);
        debug_assert!(primitive_index < batch.primitive_range.end);
        batch
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RenderOutcome {
    Presented,
    PresentedSuboptimal,
    Reconfigure,
    RecreateSurface,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceLifecycleState {
    Stable,
    ResizePending {
        requested: crate::window::WindowSize,
        stable_after: Instant,
        session_peak_area: u32,
    },
    Occluded,
    Lost,
    Unavailable,
}

pub(crate) struct WindowRenderState {
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) current_size: crate::window::WindowSize,
    pub(super) surface_state: SurfaceLifecycleState,
    pub(super) prepared_frame: Option<PreparedFrame>,
}
