mod pipelines;
mod prepare;
mod submit;
mod types;
mod upload;

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytemuck::bytes_of;
use cosmic_text::{Color as CosmicColor, SwashCache};
use wgpu::util::{DeviceExt, StagingBelt};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, ShaderStages,
    TextureViewDescriptor,
};
use winit::window::Window as WinitWindow;

use crate::error::PlatformError;
use crate::platform::wgpu::atlas::GlyphAtlas;
use crate::platform::wgpu::context::WgpuContext;
use crate::scene::CompiledScene;
use crate::style::Color;
use crate::text_system::TextSystem;
use crate::window::WindowSize;

use self::pipelines::{
    create_rect_bind_group_layout, create_text_instance_bind_group_layout,
    create_text_texture_bind_group_layout,
};
use self::types::{ColorTextInstance, RectInstance, TextInstance, ViewUniform};
use self::upload::{
    create_rect_bind_group, create_storage_buffer, rebuild_storage, stage_write_bytes,
    stage_write_pod_slice,
};

pub(crate) use self::types::{RenderOutcome, SurfaceLifecycleState, WindowRenderState};

const ATLAS_SIZE: u32 = 2048;
const STAGING_BELT_CHUNK_SIZE: u64 = 64 * 1024;
const SHRINK_IDLE_FRAME_THRESHOLD: u32 = 90;
const RESIZE_STABILITY_GRACE: Duration = Duration::from_millis(40);

pub struct RenderSystem {
    context: WgpuContext,
    staging_belt: StagingBelt,
    view_buffer: Buffer,
    view_bind_group_layout: BindGroupLayout,
    view_bind_group: BindGroup,
    rect_bind_group_layout: BindGroupLayout,
    rect_bind_group: BindGroup,
    rect_pipeline: wgpu::RenderPipeline,
    mono_text_pipeline: wgpu::RenderPipeline,
    color_text_pipeline: wgpu::RenderPipeline,
    text_texture_bind_group_layout: BindGroupLayout,
    text_instance_bind_group_layout: BindGroupLayout,
    mono_text_bind_group: BindGroup,
    color_text_bind_group: BindGroup,
    mono_atlas: GlyphAtlas,
    color_atlas: GlyphAtlas,
    swash_cache: SwashCache,
    rect_instances: Vec<RectInstance>,
    mono_text_instances: Vec<TextInstance>,
    color_text_instances: Vec<ColorTextInstance>,
    gpu_batches: Vec<types::GpuBatch>,
    rect_storage_buffer: Buffer,
    mono_text_instance_buffer: Buffer,
    color_text_instance_buffer: Buffer,
    rect_instance_capacity: usize,
    mono_text_instance_capacity: usize,
    color_text_instance_capacity: usize,
    rect_low_usage_frames: u32,
    mono_text_low_usage_frames: u32,
    color_text_low_usage_frames: u32,
    current_surface_format: Option<wgpu::TextureFormat>,
    buffer_epoch: u64,
}

impl RenderSystem {
    pub fn new(
        window: Arc<WinitWindow>,
        physical_size: WindowSize,
    ) -> Result<(Self, WindowRenderState), PlatformError> {
        let (context, surface) = WgpuContext::new(window)?;

        let view_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: Some("nekoui_view_bind_group_layout"),
                    entries: &[BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });
        let view_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nekoui_view_uniform"),
                contents: bytes_of(&ViewUniform {
                    viewport: [
                        physical_size.width.max(1) as f32,
                        physical_size.height.max(1) as f32,
                    ],
                    _pad: [0.0; 2],
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let view_bind_group = context.device.create_bind_group(&BindGroupDescriptor {
            label: Some("nekoui_view_bind_group"),
            layout: &view_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: view_buffer.as_entire_binding(),
            }],
        });
        let rect_bind_group_layout = create_rect_bind_group_layout(&context.device);
        let text_texture_bind_group_layout = create_text_texture_bind_group_layout(&context.device);
        let text_instance_bind_group_layout =
            create_text_instance_bind_group_layout(&context.device);
        let mono_atlas = GlyphAtlas::new(
            &context.device,
            &text_texture_bind_group_layout,
            crate::platform::wgpu::atlas::GlyphAtlasKind::Mono,
            ATLAS_SIZE.min(context.max_texture_size),
        )?;
        let color_atlas = GlyphAtlas::new(
            &context.device,
            &text_texture_bind_group_layout,
            crate::platform::wgpu::atlas::GlyphAtlasKind::Color,
            ATLAS_SIZE.min(context.max_texture_size),
        )?;
        let initial_extent = WindowSize::new(
            physical_size.width.max(1).min(context.max_texture_size),
            physical_size.height.max(1).min(context.max_texture_size),
        );
        let initial_surface_format = surface
            .get_default_config(
                &context.adapter,
                initial_extent.width,
                initial_extent.height,
            )
            .ok_or_else(|| PlatformError::new("surface has no default configuration"))?
            .format;
        let rect_instance_capacity = 64;
        let mono_text_instance_capacity = 256;
        let color_text_instance_capacity = 64;
        let rect_storage_buffer = create_storage_buffer::<RectInstance>(
            &context.device,
            "nekoui_rect_instances",
            rect_instance_capacity,
        );
        let rect_bind_group = create_rect_bind_group(
            &context.device,
            &rect_bind_group_layout,
            &rect_storage_buffer,
        );
        let (mono_text_instance_buffer, mono_text_bind_group) = rebuild_storage::<TextInstance>(
            &context.device,
            &text_instance_bind_group_layout,
            mono_text_instance_capacity,
            "nekoui_mono_text_instances",
            "nekoui_mono_text_bind_group",
        );
        let (color_text_instance_buffer, color_text_bind_group) =
            rebuild_storage::<ColorTextInstance>(
                &context.device,
                &text_instance_bind_group_layout,
                color_text_instance_capacity,
                "nekoui_color_text_instances",
                "nekoui_color_text_bind_group",
            );
        let staging_device = context.device.clone();
        let rect_pipeline = pipelines::create_rect_pipeline(
            &context.device,
            &view_bind_group_layout,
            &rect_bind_group_layout,
            initial_surface_format,
        );
        let mono_text_pipeline = pipelines::create_mono_text_pipeline(
            &context.device,
            &view_bind_group_layout,
            &text_texture_bind_group_layout,
            &text_instance_bind_group_layout,
            initial_surface_format,
        );
        let color_text_pipeline = pipelines::create_color_text_pipeline(
            &context.device,
            &view_bind_group_layout,
            &text_texture_bind_group_layout,
            &text_instance_bind_group_layout,
            initial_surface_format,
        );

        let mut render_system = Self {
            context,
            staging_belt: StagingBelt::new(staging_device, STAGING_BELT_CHUNK_SIZE),
            view_buffer,
            view_bind_group_layout,
            view_bind_group,
            rect_bind_group_layout,
            rect_bind_group,
            rect_pipeline,
            mono_text_pipeline,
            color_text_pipeline,
            text_texture_bind_group_layout,
            text_instance_bind_group_layout,
            mono_text_bind_group,
            color_text_bind_group,
            mono_atlas,
            color_atlas,
            swash_cache: SwashCache::new(),
            rect_instances: Vec::new(),
            mono_text_instances: Vec::new(),
            color_text_instances: Vec::new(),
            gpu_batches: Vec::new(),
            rect_storage_buffer,
            mono_text_instance_buffer,
            color_text_instance_buffer,
            rect_instance_capacity,
            mono_text_instance_capacity,
            color_text_instance_capacity,
            rect_low_usage_frames: 0,
            mono_text_low_usage_frames: 0,
            color_text_low_usage_frames: 0,
            current_surface_format: Some(initial_surface_format),
            buffer_epoch: 1,
        };
        let render_state = render_system.create_window_state(surface, physical_size)?;
        Ok((render_system, render_state))
    }

    pub fn create_surface_for_window(
        &self,
        window: Arc<WinitWindow>,
    ) -> Result<wgpu::Surface<'static>, PlatformError> {
        self.context.create_surface_for_window(window)
    }

    pub fn create_window_state(
        &mut self,
        surface: wgpu::Surface<'static>,
        physical_size: WindowSize,
    ) -> Result<WindowRenderState, PlatformError> {
        let current_size = physical_size;
        let physical_size = self.surface_extent_for(physical_size);
        let mut config = surface
            .get_default_config(
                &self.context.adapter,
                physical_size.width,
                physical_size.height,
            )
            .ok_or_else(|| PlatformError::new("surface has no default configuration"))?;
        config.desired_maximum_frame_latency = 1;
        surface.configure(&self.context.device, &config);
        self.ensure_pipelines_for_format(config.format);
        Ok(WindowRenderState {
            surface,
            config,
            current_size,
            surface_state: SurfaceLifecycleState::Stable,
            prepared_frame: None,
        })
    }

    pub fn note_surface_resize(&self, state: &mut WindowRenderState, physical_size: WindowSize) {
        state.current_size = physical_size;
        if physical_size.width == 0 || physical_size.height == 0 {
            state.surface_state = SurfaceLifecycleState::Unavailable;
            return;
        }

        let peak_area = match state.surface_state {
            SurfaceLifecycleState::ResizePending {
                session_peak_area, ..
            } => session_peak_area.max(physical_size.width.saturating_mul(physical_size.height)),
            _ => physical_size.width.saturating_mul(physical_size.height),
        };
        state.surface_state = SurfaceLifecycleState::ResizePending {
            requested: physical_size,
            stable_after: Instant::now() + RESIZE_STABILITY_GRACE,
            session_peak_area: peak_area,
        };
    }

    pub fn note_surface_occlusion(&self, state: &mut WindowRenderState, occluded: bool) {
        if occluded {
            state.surface_state = SurfaceLifecycleState::Occluded;
            return;
        }

        if state.current_size.width == 0 || state.current_size.height == 0 {
            state.surface_state = SurfaceLifecycleState::Unavailable;
            return;
        }

        let target_size = self.surface_extent_for(state.current_size);
        state.surface_state = if state.config.width == target_size.width
            && state.config.height == target_size.height
        {
            SurfaceLifecycleState::Stable
        } else {
            SurfaceLifecycleState::ResizePending {
                requested: state.current_size,
                stable_after: Instant::now() + RESIZE_STABILITY_GRACE,
                session_peak_area: state
                    .current_size
                    .width
                    .saturating_mul(state.current_size.height),
            }
        };
    }

    pub fn recreate_surface(
        &mut self,
        state: &mut WindowRenderState,
        window: Arc<WinitWindow>,
    ) -> Result<(), PlatformError> {
        state.surface = self.context.create_surface_for_window(window)?;
        self.note_surface_resize(state, state.current_size);
        Ok(())
    }

    pub fn render(
        &mut self,
        state: &mut WindowRenderState,
        scene: &CompiledScene,
        text_system: &mut TextSystem,
        window: &WinitWindow,
        scale_factor: f64,
    ) -> Result<RenderOutcome, PlatformError> {
        if state.current_size.width == 0 || state.current_size.height == 0 {
            state.surface_state = SurfaceLifecycleState::Unavailable;
            return Ok(RenderOutcome::Unavailable);
        }

        self.configure_surface_if_needed(state)?;

        self.prepare_frame(state, scene, text_system, scale_factor as f32);
        let prepared = state
            .prepared_frame
            .as_ref()
            .expect("prepared frame must exist before rendering");
        self.ensure_rect_capacity(prepared.rect_instances.len());
        self.ensure_mono_text_capacity(prepared.mono_text_instances.len());
        self.ensure_color_text_capacity(prepared.color_text_instances.len());
        self.maybe_shrink_rect_capacity(prepared.rect_instances.len());
        self.maybe_shrink_mono_text_capacity(prepared.mono_text_instances.len());
        self.maybe_shrink_color_text_capacity(prepared.color_text_instances.len());
        let uploads_required = state
            .prepared_frame
            .as_ref()
            .is_some_and(|prepared| prepared.uploaded_buffer_epoch != self.buffer_epoch);

        let (frame, presented_suboptimal) = match state.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.note_surface_resize(state, state.current_size);
                return Ok(RenderOutcome::Reconfigure);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                state.surface_state = SurfaceLifecycleState::Lost;
                return Ok(RenderOutcome::RecreateSurface);
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                return Ok(RenderOutcome::Unavailable);
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                state.surface_state = SurfaceLifecycleState::Occluded;
                return Ok(RenderOutcome::Unavailable);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(PlatformError::new(
                    "surface validation failed during get_current_texture",
                ));
            }
        };

        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("nekoui_encoder"),
                });

        stage_write_bytes(
            &mut self.staging_belt,
            &mut encoder,
            &self.view_buffer,
            bytes_of(&ViewUniform {
                viewport: [state.config.width as f32, state.config.height as f32],
                _pad: [0.0; 2],
            }),
        );
        if uploads_required {
            let prepared = state
                .prepared_frame
                .as_ref()
                .expect("prepared frame must exist for uploads");
            stage_write_pod_slice(
                &mut self.staging_belt,
                &mut encoder,
                &self.rect_storage_buffer,
                &prepared.rect_instances,
            );
            stage_write_pod_slice(
                &mut self.staging_belt,
                &mut encoder,
                &self.mono_text_instance_buffer,
                &prepared.mono_text_instances,
            );
            stage_write_pod_slice(
                &mut self.staging_belt,
                &mut encoder,
                &self.color_text_instance_buffer,
                &prepared.color_text_instances,
            );
        }
        self.staging_belt.finish();

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("nekoui_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color_to_wgpu(
                            scene.clear_color.unwrap_or(Color::rgba(1.0, 1.0, 1.0, 1.0)),
                        )),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            let mut current_submit_state = None;
            for batch in &*state
                .prepared_frame
                .as_ref()
                .expect("prepared frame must exist during submit")
                .gpu_batches
            {
                let submit_state = types::BatchSubmitState::from(batch);
                if current_submit_state != Some(submit_state) {
                    if current_submit_state.map(|state| state.pipeline_key)
                        != Some(submit_state.pipeline_key)
                    {
                        match submit_state.pipeline_key {
                            types::PipelineKey::Rect => {
                                pass.set_pipeline(&self.rect_pipeline);
                                pass.set_bind_group(0, &self.view_bind_group, &[]);
                                pass.set_bind_group(1, &self.rect_bind_group, &[]);
                            }
                            types::PipelineKey::MonoText => {
                                pass.set_pipeline(&self.mono_text_pipeline);
                                pass.set_bind_group(0, &self.view_bind_group, &[]);
                                pass.set_bind_group(2, &self.mono_text_bind_group, &[]);
                            }
                            types::PipelineKey::ColorText => {
                                pass.set_pipeline(&self.color_text_pipeline);
                                pass.set_bind_group(0, &self.view_bind_group, &[]);
                                pass.set_bind_group(2, &self.color_text_bind_group, &[]);
                            }
                        }
                    }

                    if current_submit_state.map(|state| state.texture_binding)
                        != Some(submit_state.texture_binding)
                    {
                        match submit_state.texture_binding {
                            types::TextureBindingKey::None => {}
                            types::TextureBindingKey::MonoGlyphAtlas(page_id) => {
                                let Some(bind_group) = self.mono_atlas.bind_group(page_id) else {
                                    continue;
                                };
                                pass.set_bind_group(1, bind_group, &[]);
                            }
                            types::TextureBindingKey::ColorGlyphAtlas(page_id) => {
                                let Some(bind_group) = self.color_atlas.bind_group(page_id) else {
                                    continue;
                                };
                                pass.set_bind_group(1, bind_group, &[]);
                            }
                        }
                    }

                    match submit_state.clip_policy {
                        types::BatchClipPolicy::None => {
                            pass.set_scissor_rect(0, 0, state.config.width, state.config.height);
                        }
                        types::BatchClipPolicy::Bounds => {
                            let Some(scissor) = submit::clip_bounds_to_scissor_rect(
                                submit_state.clip_stack,
                                scale_factor as f32,
                                state.config.width,
                                state.config.height,
                            ) else {
                                continue;
                            };
                            pass.set_scissor_rect(
                                scissor.x,
                                scissor.y,
                                scissor.width,
                                scissor.height,
                            );
                        }
                    }

                    current_submit_state = Some(submit_state);
                }
                match submit_state.effect_render_policy {
                    types::EffectRenderPolicy::Direct => {
                        submit::draw_gpu_batch(&mut pass, batch.instance_range.clone());
                    }
                    types::EffectRenderPolicy::InlineOpacity => {
                        submit::draw_gpu_batch_inline_opacity(
                            &mut pass,
                            batch.instance_range.clone(),
                        );
                    }
                }
            }
        }

        window.pre_present_notify();
        self.context.queue.submit(Some(encoder.finish()));
        self.staging_belt.recall();
        if uploads_required && let Some(prepared) = state.prepared_frame.as_mut() {
            prepared.uploaded_buffer_epoch = self.buffer_epoch;
        }
        frame.present();
        if presented_suboptimal {
            self.note_surface_resize(state, state.current_size);
            Ok(RenderOutcome::PresentedSuboptimal)
        } else {
            Ok(RenderOutcome::Presented)
        }
    }

    fn configure_surface_if_needed(
        &mut self,
        state: &mut WindowRenderState,
    ) -> Result<(), PlatformError> {
        let target_size = self.surface_extent_for(state.current_size);
        let configured_matches =
            state.config.width == target_size.width && state.config.height == target_size.height;

        let needs_configure = match state.surface_state {
            SurfaceLifecycleState::ResizePending { .. } | SurfaceLifecycleState::Lost => true,
            SurfaceLifecycleState::Occluded | SurfaceLifecycleState::Unavailable => {
                !configured_matches
            }
            SurfaceLifecycleState::Stable => !configured_matches,
        };

        if !needs_configure {
            state.surface_state = SurfaceLifecycleState::Stable;
            return Ok(());
        }

        state.config.width = target_size.width;
        state.config.height = target_size.height;
        state.surface.configure(&self.context.device, &state.config);
        self.ensure_pipelines_for_format(state.config.format);
        state.surface_state = SurfaceLifecycleState::Stable;
        Ok(())
    }

    fn surface_extent_for(&self, physical_size: WindowSize) -> WindowSize {
        let max = self.context.max_texture_size;
        WindowSize::new(
            physical_size.width.max(1).min(max),
            physical_size.height.max(1).min(max),
        )
    }
}

fn cosmic_to_style_color(color: CosmicColor) -> Color {
    Color::rgba(
        f32::from(color.r()) / 255.0,
        f32::from(color.g()) / 255.0,
        f32::from(color.b()) / 255.0,
        f32::from(color.a()) / 255.0,
    )
}

fn color_to_wgpu(color: Color) -> wgpu::Color {
    wgpu::Color {
        r: color.r as f64,
        g: color.g as f64,
        b: color.b as f64,
        a: color.a as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::submit::{
        can_merge_gpu_batches, clip_bounds_to_scissor_rect, effect_render_policy, push_gpu_batch,
    };
    use super::types::{
        BatchClipPolicy, BatchEffectPolicy, BatchSubmitState, ClipStack, ColorTextInstance,
        EffectRenderPolicy, GpuBatch, PipelineKey, ScissorRect, TextInstance, TextureBindingKey,
    };
    use crate::scene::{ClipShape, EffectClass, LayoutBox};

    fn rect_clip(bounds: LayoutBox) -> ClipStack {
        ClipStack {
            first: Some(ClipShape::Rect(bounds)),
            second: None,
        }
    }

    #[test]
    fn text_storage_instances_match_wgsl_stride() {
        assert_eq!(std::mem::size_of::<TextInstance>(), 112);
        assert_eq!(std::mem::size_of::<ColorTextInstance>(), 112);
    }

    #[test]
    fn gpu_batch_merge_respects_clip_and_effect_boundaries() {
        let mut batches = Vec::new();
        push_gpu_batch(
            &mut batches,
            GpuBatch {
                pipeline_key: PipelineKey::MonoText,
                texture_binding: TextureBindingKey::MonoGlyphAtlas(0),
                clip_stack: ClipStack::default(),
                effect_class: EffectClass::None,
                instance_range: 0..4,
            },
        );
        push_gpu_batch(
            &mut batches,
            GpuBatch {
                pipeline_key: PipelineKey::MonoText,
                texture_binding: TextureBindingKey::MonoGlyphAtlas(0),
                clip_stack: rect_clip(LayoutBox {
                    x: 10.0,
                    y: 20.0,
                    width: 40.0,
                    height: 50.0,
                }),
                effect_class: EffectClass::None,
                instance_range: 4..8,
            },
        );
        push_gpu_batch(
            &mut batches,
            GpuBatch {
                pipeline_key: PipelineKey::MonoText,
                texture_binding: TextureBindingKey::MonoGlyphAtlas(0),
                clip_stack: rect_clip(LayoutBox {
                    x: 10.0,
                    y: 20.0,
                    width: 40.0,
                    height: 50.0,
                }),
                effect_class: EffectClass::Opacity,
                instance_range: 8..12,
            },
        );

        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].instance_range, 0..4);
        assert_eq!(batches[1].instance_range, 4..8);
        assert_eq!(batches[2].instance_range, 8..12);
        assert!(!can_merge_gpu_batches(&batches[0], &batches[1]));
        assert!(!can_merge_gpu_batches(&batches[1], &batches[2]));
    }

    #[test]
    fn gpu_batch_merge_coalesces_compatible_adjacent_ranges() {
        let mut batches = Vec::new();
        push_gpu_batch(
            &mut batches,
            GpuBatch {
                pipeline_key: PipelineKey::Rect,
                texture_binding: TextureBindingKey::None,
                clip_stack: rect_clip(LayoutBox {
                    x: 4.0,
                    y: 8.0,
                    width: 16.0,
                    height: 12.0,
                }),
                effect_class: EffectClass::Opacity,
                instance_range: 0..2,
            },
        );
        push_gpu_batch(
            &mut batches,
            GpuBatch {
                pipeline_key: PipelineKey::Rect,
                texture_binding: TextureBindingKey::None,
                clip_stack: rect_clip(LayoutBox {
                    x: 4.0,
                    y: 8.0,
                    width: 16.0,
                    height: 12.0,
                }),
                effect_class: EffectClass::Opacity,
                instance_range: 2..5,
            },
        );

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].instance_range, 0..5);
    }

    #[test]
    fn batch_submit_state_tracks_clip_and_effect_policy() {
        let batch = GpuBatch {
            pipeline_key: PipelineKey::ColorText,
            texture_binding: TextureBindingKey::ColorGlyphAtlas(3),
            clip_stack: rect_clip(LayoutBox {
                x: 1.0,
                y: 2.0,
                width: 30.0,
                height: 40.0,
            }),
            effect_class: EffectClass::Opacity,
            instance_range: 3..9,
        };

        let submit_state = BatchSubmitState::from(&batch);
        assert_eq!(submit_state.pipeline_key, PipelineKey::ColorText);
        assert_eq!(
            submit_state.texture_binding,
            TextureBindingKey::ColorGlyphAtlas(3)
        );
        assert_eq!(submit_state.clip_policy, BatchClipPolicy::Bounds);
        assert_eq!(
            submit_state.clip_stack,
            rect_clip(LayoutBox {
                x: 1.0,
                y: 2.0,
                width: 30.0,
                height: 40.0,
            })
        );
        assert_eq!(submit_state.effect_policy, BatchEffectPolicy::Opacity);
        assert_eq!(
            submit_state.effect_render_policy,
            EffectRenderPolicy::InlineOpacity
        );
    }

    #[test]
    fn gpu_batch_merge_respects_clip_bounds() {
        let mut batches = Vec::new();
        push_gpu_batch(
            &mut batches,
            GpuBatch {
                pipeline_key: PipelineKey::Rect,
                texture_binding: TextureBindingKey::None,
                clip_stack: rect_clip(LayoutBox {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                }),
                effect_class: EffectClass::None,
                instance_range: 0..2,
            },
        );
        push_gpu_batch(
            &mut batches,
            GpuBatch {
                pipeline_key: PipelineKey::Rect,
                texture_binding: TextureBindingKey::None,
                clip_stack: rect_clip(LayoutBox {
                    x: 2.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                }),
                effect_class: EffectClass::None,
                instance_range: 2..4,
            },
        );

        assert_eq!(batches.len(), 2);
        assert!(!can_merge_gpu_batches(&batches[0], &batches[1]));
    }

    #[test]
    fn gpu_batch_merge_respects_atlas_page_boundaries() {
        let mut batches = Vec::new();
        push_gpu_batch(
            &mut batches,
            GpuBatch {
                pipeline_key: PipelineKey::MonoText,
                texture_binding: TextureBindingKey::MonoGlyphAtlas(1),
                clip_stack: ClipStack::default(),
                effect_class: EffectClass::None,
                instance_range: 0..2,
            },
        );
        push_gpu_batch(
            &mut batches,
            GpuBatch {
                pipeline_key: PipelineKey::MonoText,
                texture_binding: TextureBindingKey::MonoGlyphAtlas(2),
                clip_stack: ClipStack::default(),
                effect_class: EffectClass::None,
                instance_range: 2..4,
            },
        );

        assert_eq!(batches.len(), 2);
        assert!(!can_merge_gpu_batches(&batches[0], &batches[1]));
    }

    #[test]
    fn clip_bounds_to_scissor_rect_clamps_to_viewport() {
        let scissor = clip_bounds_to_scissor_rect(
            rect_clip(LayoutBox {
                x: -4.25,
                y: 2.25,
                width: 20.75,
                height: 40.5,
            }),
            2.0,
            24,
            32,
        )
        .unwrap();

        assert_eq!(
            scissor,
            ScissorRect {
                x: 0,
                y: 4,
                width: 24,
                height: 28,
            }
        );
    }

    #[test]
    fn effect_policy_maps_to_real_render_policy() {
        assert_eq!(
            effect_render_policy(BatchEffectPolicy::None),
            EffectRenderPolicy::Direct
        );
        assert_eq!(
            effect_render_policy(BatchEffectPolicy::Opacity),
            EffectRenderPolicy::InlineOpacity
        );
    }
}
