use std::ops::Range;

use super::types::{BatchEffectPolicy, EffectRenderPolicy, GpuBatch, GpuBatchBuilder, ScissorRect};

pub(super) fn effect_render_policy(effect_policy: BatchEffectPolicy) -> EffectRenderPolicy {
    match effect_policy {
        BatchEffectPolicy::None => EffectRenderPolicy::Direct,
        BatchEffectPolicy::Opacity => EffectRenderPolicy::InlineOpacity,
    }
}

pub(super) fn can_merge_gpu_batches(previous: &GpuBatch, next: &GpuBatch) -> bool {
    previous.pipeline_key == next.pipeline_key
        && previous.texture_binding == next.texture_binding
        && previous.clip_stack == next.clip_stack
        && previous.effect_class == next.effect_class
        && previous.instance_range.end == next.instance_range.start
}

pub(super) fn push_gpu_batch(batches: &mut Vec<GpuBatch>, batch: GpuBatch) {
    let mut builder = GpuBatchBuilder {
        batches: std::mem::take(batches),
    };
    builder.push(batch);
    *batches = builder.batches;
}

pub(super) fn clip_bounds_to_scissor_rect(
    clip_stack: &super::types::ClipStack,
    scale_factor: f32,
    viewport_width: u32,
    viewport_height: u32,
) -> Option<ScissorRect> {
    let clip_bounds = clip_stack.scissor_bounds()?;
    let scale_factor = scale_factor.max(f32::MIN_POSITIVE);
    let left = (clip_bounds.x * scale_factor).floor().max(0.0);
    let top = (clip_bounds.y * scale_factor).floor().max(0.0);
    let right = ((clip_bounds.x + clip_bounds.width) * scale_factor)
        .ceil()
        .min(viewport_width as f32);
    let bottom = ((clip_bounds.y + clip_bounds.height) * scale_factor)
        .ceil()
        .min(viewport_height as f32);

    if right <= left || bottom <= top {
        return None;
    }

    Some(ScissorRect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

pub(super) fn draw_gpu_batch(pass: &mut wgpu::RenderPass<'_>, instance_range: Range<u32>) {
    pass.draw(0..6, instance_range);
}

pub(super) fn draw_gpu_batch_inline_opacity(
    pass: &mut wgpu::RenderPass<'_>,
    instance_range: Range<u32>,
) {
    pass.draw(0..6, instance_range);
}
