use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use cosmic_text::{CacheKey, Color as CosmicColor, SwashCache};
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BlendState, Buffer, BufferBindingType, ColorTargetState,
    ColorWrites, Device, FragmentState, LoadOp, MultisampleState, Operations,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, SurfaceConfiguration,
    TextureFormat, TextureViewDescriptor, VertexBufferLayout, VertexState, VertexStepMode,
    vertex_attr_array,
};
use winit::window::Window as WinitWindow;

use crate::error::PlatformError;
use crate::platform::wgpu::atlas::{AtlasEntry, GlyphAtlas, GlyphAtlasKind};
use crate::platform::wgpu::context::WgpuContext;
use crate::scene::{CompiledScene, MaterialClass, Primitive, SceneNodeId};
use crate::style::Color;
use crate::text_system::TextSystem;
use crate::window::WindowSize;

const ATLAS_SIZE: u32 = 2048;

const QUAD_SHADER: &str = include_str!("shader/quad.wgsl");
const TEXT_SHADER: &str = include_str!("shader/text.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ViewUniform {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadInstance {
    rect: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TextInstance {
    rect: [f32; 4],
    uv_rect: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ColorTextInstance {
    rect: [f32; 4],
    uv_rect: [f32; 4],
    alpha: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct PrimitiveSceneState {
    offset: [f32; 2],
    opacity: f32,
    clip: Option<crate::scene::LayoutBox>,
}

#[derive(Debug, Clone, Copy)]
pub enum RenderOutcome {
    Presented,
    Reconfigure,
    Skip,
}

pub struct WindowRenderState {
    surface: wgpu::Surface<'static>,
    config: SurfaceConfiguration,
}

pub struct RenderSystem {
    context: WgpuContext,
    view_buffer: Buffer,
    view_bind_group_layout: BindGroupLayout,
    view_bind_group: BindGroup,
    quad_pipeline: RenderPipeline,
    mono_text_pipeline: RenderPipeline,
    color_text_pipeline: RenderPipeline,
    text_texture_bind_group_layout: BindGroupLayout,
    mono_atlas: GlyphAtlas,
    color_atlas: GlyphAtlas,
    swash_cache: SwashCache,
    quad_instances: Vec<QuadInstance>,
    mono_text_instances: Vec<TextInstance>,
    color_text_instances: Vec<ColorTextInstance>,
    quad_instance_buffer: Buffer,
    mono_text_instance_buffer: Buffer,
    color_text_instance_buffer: Buffer,
    quad_instance_capacity: usize,
    mono_text_instance_capacity: usize,
    color_text_instance_capacity: usize,
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
                contents: bytemuck::bytes_of(&ViewUniform {
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

        let text_texture_bind_group_layout = create_text_texture_bind_group_layout(&context.device);
        let mono_atlas = GlyphAtlas::new(
            &context.device,
            &text_texture_bind_group_layout,
            GlyphAtlasKind::Mono,
            ATLAS_SIZE.min(context.max_texture_size),
        )?;
        let color_atlas = GlyphAtlas::new(
            &context.device,
            &text_texture_bind_group_layout,
            GlyphAtlasKind::Color,
            ATLAS_SIZE.min(context.max_texture_size),
        )?;
        let quad_pipeline = create_quad_pipeline(
            &context.device,
            &view_bind_group_layout,
            TextureFormat::Bgra8UnormSrgb,
        );
        let mono_text_pipeline = create_mono_text_pipeline(
            &context.device,
            &view_bind_group_layout,
            &text_texture_bind_group_layout,
            TextureFormat::Bgra8UnormSrgb,
        );
        let color_text_pipeline = create_color_text_pipeline(
            &context.device,
            &view_bind_group_layout,
            &text_texture_bind_group_layout,
            TextureFormat::Bgra8UnormSrgb,
        );

        let quad_instance_capacity = 64;
        let mono_text_instance_capacity = 256;
        let color_text_instance_capacity = 64;
        let quad_instance_buffer = create_instance_buffer::<QuadInstance>(
            &context.device,
            "nekoui_quad_instances",
            quad_instance_capacity,
        );
        let mono_text_instance_buffer = create_instance_buffer::<TextInstance>(
            &context.device,
            "nekoui_mono_text_instances",
            mono_text_instance_capacity,
        );
        let color_text_instance_buffer = create_instance_buffer::<ColorTextInstance>(
            &context.device,
            "nekoui_color_text_instances",
            color_text_instance_capacity,
        );

        let mut render_system = Self {
            context,
            view_buffer,
            view_bind_group_layout,
            view_bind_group,
            quad_pipeline,
            mono_text_pipeline,
            color_text_pipeline,
            text_texture_bind_group_layout,
            mono_atlas,
            color_atlas,
            swash_cache: SwashCache::new(),
            quad_instances: Vec::new(),
            mono_text_instances: Vec::new(),
            color_text_instances: Vec::new(),
            quad_instance_buffer,
            mono_text_instance_buffer,
            color_text_instance_buffer,
            quad_instance_capacity,
            mono_text_instance_capacity,
            color_text_instance_capacity,
        };
        let render_state = render_system.create_window_state(surface, physical_size)?;
        render_system.rebuild_pipelines(render_state.config.format);
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
        let physical_size = self.surface_extent_for(physical_size);
        let config = surface
            .get_default_config(
                &self.context.adapter,
                physical_size.width,
                physical_size.height,
            )
            .ok_or_else(|| PlatformError::new("surface has no default configuration"))?;
        surface.configure(&self.context.device, &config);
        self.rebuild_pipelines(config.format);
        Ok(WindowRenderState { surface, config })
    }

    pub fn resize(
        &mut self,
        state: &mut WindowRenderState,
        physical_size: WindowSize,
    ) -> Result<(), PlatformError> {
        if physical_size.width == 0 || physical_size.height == 0 {
            return Ok(());
        }

        let physical_size = self.surface_extent_for(physical_size);
        if state.config.width == physical_size.width && state.config.height == physical_size.height
        {
            return Ok(());
        }

        state.config.width = physical_size.width;
        state.config.height = physical_size.height;
        state.surface.configure(&self.context.device, &state.config);
        self.rebuild_pipelines(state.config.format);
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
        if state.config.width == 0 || state.config.height == 0 {
            return Ok(RenderOutcome::Skip);
        }

        self.context.queue.write_buffer(
            &self.view_buffer,
            0,
            bytemuck::bytes_of(&ViewUniform {
                viewport: [state.config.width as f32, state.config.height as f32],
                _pad: [0.0; 2],
            }),
        );

        self.quad_instances.clear();
        self.mono_text_instances.clear();
        self.color_text_instances.clear();
        self.collect_instances(scene, text_system, scale_factor as f32);
        self.ensure_quad_capacity(self.quad_instances.len());
        self.ensure_mono_text_capacity(self.mono_text_instances.len());
        self.ensure_color_text_capacity(self.color_text_instances.len());

        if !self.quad_instances.is_empty() {
            self.context.queue.write_buffer(
                &self.quad_instance_buffer,
                0,
                bytemuck::cast_slice(&self.quad_instances),
            );
        }
        if !self.mono_text_instances.is_empty() {
            self.context.queue.write_buffer(
                &self.mono_text_instance_buffer,
                0,
                bytemuck::cast_slice(&self.mono_text_instances),
            );
        }
        if !self.color_text_instances.is_empty() {
            self.context.queue.write_buffer(
                &self.color_text_instance_buffer,
                0,
                bytemuck::cast_slice(&self.color_text_instances),
            );
        }

        let frame = match state.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                state.surface.configure(&self.context.device, &state.config);
                return Ok(RenderOutcome::Reconfigure);
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(RenderOutcome::Skip),
        };

        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("nekoui_encoder"),
                });

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("nekoui_render_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(color_to_wgpu(
                            scene.clear_color.unwrap_or(Color::rgba(1.0, 1.0, 1.0, 1.0)),
                        )),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            if !self.quad_instances.is_empty() {
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_bind_group(0, &self.view_bind_group, &[]);
                pass.set_vertex_buffer(0, self.quad_instance_buffer.slice(..));
                pass.draw(0..6, 0..self.quad_instances.len() as u32);
            }

            if !self.mono_text_instances.is_empty() {
                pass.set_pipeline(&self.mono_text_pipeline);
                pass.set_bind_group(0, &self.view_bind_group, &[]);
                pass.set_bind_group(1, self.mono_atlas.bind_group(), &[]);
                pass.set_vertex_buffer(0, self.mono_text_instance_buffer.slice(..));
                pass.draw(0..6, 0..self.mono_text_instances.len() as u32);
            }

            if !self.color_text_instances.is_empty() {
                pass.set_pipeline(&self.color_text_pipeline);
                pass.set_bind_group(0, &self.view_bind_group, &[]);
                pass.set_bind_group(1, self.color_atlas.bind_group(), &[]);
                pass.set_vertex_buffer(0, self.color_text_instance_buffer.slice(..));
                pass.draw(0..6, 0..self.color_text_instances.len() as u32);
            }
        }

        window.pre_present_notify();
        self.context.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(RenderOutcome::Presented)
    }

    fn collect_instances(
        &mut self,
        scene: &CompiledScene,
        text_system: &mut TextSystem,
        scale_factor: f32,
    ) {
        let scale_factor = scale_factor.max(f32::MIN_POSITIVE);
        let primitive_states = collect_primitive_states(scene);
        for batch in scene.logical_batches.iter() {
            match batch.material_class {
                MaterialClass::Quad => {
                    for (primitive_index, primitive) in scene.primitives
                        [batch.primitive_range.as_range()]
                    .iter()
                    .enumerate()
                    {
                        let primitive_index =
                            batch.primitive_range.start as usize + primitive_index;
                        let state = primitive_states[primitive_index];
                        if let Primitive::Quad { bounds, color } = primitive {
                            let rect = crate::scene::LayoutBox {
                                x: bounds.x + state.offset[0],
                                y: bounds.y + state.offset[1],
                                width: bounds.width,
                                height: bounds.height,
                            };
                            let Some(clipped_rect) = clip_rect(rect, state.clip) else {
                                continue;
                            };
                            self.quad_instances.push(QuadInstance {
                                rect: [
                                    clipped_rect.x * scale_factor,
                                    clipped_rect.y * scale_factor,
                                    clipped_rect.width * scale_factor,
                                    clipped_rect.height * scale_factor,
                                ],
                                color: [color.r, color.g, color.b, color.a * state.opacity],
                            });
                        }
                    }
                }
                MaterialClass::Text => {
                    for (primitive_index, primitive) in scene.primitives
                        [batch.primitive_range.as_range()]
                    .iter()
                    .enumerate()
                    {
                        let primitive_index =
                            batch.primitive_range.start as usize + primitive_index;
                        let state = primitive_states[primitive_index];
                        let Primitive::Text {
                            bounds,
                            layout,
                            color,
                        } = primitive
                        else {
                            continue;
                        };

                        for run in &layout.runs {
                            for glyph in &run.glyphs {
                                let physical = glyph.physical(
                                    (
                                        (bounds.x + state.offset[0]) * scale_factor,
                                        (bounds.y + state.offset[1] + run.baseline) * scale_factor,
                                    ),
                                    scale_factor,
                                );
                                let Some((atlas_kind, entry)) =
                                    self.ensure_glyph_entry(text_system, physical.cache_key)
                                else {
                                    continue;
                                };
                                let rect = crate::scene::LayoutBox {
                                    x: (physical.x + entry.placement_left) as f32,
                                    y: (physical.y - entry.placement_top) as f32,
                                    width: entry.width as f32,
                                    height: entry.height as f32,
                                };
                                let uv = crate::scene::LayoutBox {
                                    x: entry.uv_rect[0],
                                    y: entry.uv_rect[1],
                                    width: entry.uv_rect[2],
                                    height: entry.uv_rect[3],
                                };
                                let Some((clipped_rect, clipped_uv)) =
                                    clip_text_glyph(rect, uv, state.clip)
                                else {
                                    continue;
                                };
                                let rect = [
                                    clipped_rect.x,
                                    clipped_rect.y,
                                    clipped_rect.width,
                                    clipped_rect.height,
                                ];

                                match atlas_kind {
                                    GlyphAtlasKind::Mono => {
                                        let glyph_color = glyph
                                            .color_opt
                                            .map(cosmic_to_style_color)
                                            .unwrap_or(*color);
                                        self.mono_text_instances.push(TextInstance {
                                            rect,
                                            uv_rect: [
                                                clipped_uv.x,
                                                clipped_uv.y,
                                                clipped_uv.width,
                                                clipped_uv.height,
                                            ],
                                            color: [
                                                glyph_color.r,
                                                glyph_color.g,
                                                glyph_color.b,
                                                glyph_color.a * state.opacity,
                                            ],
                                        });
                                    }
                                    GlyphAtlasKind::Color => {
                                        let alpha = glyph
                                            .color_opt
                                            .map(cosmic_to_style_color)
                                            .unwrap_or(*color)
                                            .a;
                                        self.color_text_instances.push(ColorTextInstance {
                                            rect,
                                            uv_rect: [
                                                clipped_uv.x,
                                                clipped_uv.y,
                                                clipped_uv.width,
                                                clipped_uv.height,
                                            ],
                                            alpha: alpha * state.opacity,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn ensure_glyph_entry(
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
            cosmic_text::SwashContent::Color => self
                .color_atlas
                .upload_color(&self.context.queue, cache_key, &image)
                .map(|entry| (GlyphAtlasKind::Color, entry)),
            cosmic_text::SwashContent::Mask | cosmic_text::SwashContent::SubpixelMask => self
                .mono_atlas
                .upload_mask(&self.context.queue, cache_key, &image)
                .map(|entry| (GlyphAtlasKind::Mono, entry)),
        }
    }

    fn surface_extent_for(&self, physical_size: WindowSize) -> WindowSize {
        let max = self.context.max_texture_size;
        WindowSize::new(
            physical_size.width.max(1).min(max),
            physical_size.height.max(1).min(max),
        )
    }

    fn rebuild_pipelines(&mut self, surface_format: TextureFormat) {
        self.quad_pipeline = create_quad_pipeline(
            &self.context.device,
            &self.view_bind_group_layout,
            surface_format,
        );
        self.mono_text_pipeline = create_mono_text_pipeline(
            &self.context.device,
            &self.view_bind_group_layout,
            &self.text_texture_bind_group_layout,
            surface_format,
        );
        self.color_text_pipeline = create_color_text_pipeline(
            &self.context.device,
            &self.view_bind_group_layout,
            &self.text_texture_bind_group_layout,
            surface_format,
        );
    }

    fn ensure_quad_capacity(&mut self, count: usize) {
        if count <= self.quad_instance_capacity {
            return;
        }
        while self.quad_instance_capacity < count {
            self.quad_instance_capacity *= 2;
        }
        self.quad_instance_buffer = create_instance_buffer::<QuadInstance>(
            &self.context.device,
            "nekoui_quad_instances",
            self.quad_instance_capacity,
        );
    }

    fn ensure_mono_text_capacity(&mut self, count: usize) {
        if count <= self.mono_text_instance_capacity {
            return;
        }
        while self.mono_text_instance_capacity < count {
            self.mono_text_instance_capacity *= 2;
        }
        self.mono_text_instance_buffer = create_instance_buffer::<TextInstance>(
            &self.context.device,
            "nekoui_mono_text_instances",
            self.mono_text_instance_capacity,
        );
    }

    fn ensure_color_text_capacity(&mut self, count: usize) {
        if count <= self.color_text_instance_capacity {
            return;
        }
        while self.color_text_instance_capacity < count {
            self.color_text_instance_capacity *= 2;
        }
        self.color_text_instance_buffer = create_instance_buffer::<ColorTextInstance>(
            &self.context.device,
            "nekoui_color_text_instances",
            self.color_text_instance_capacity,
        );
    }
}

fn collect_primitive_states(scene: &CompiledScene) -> Vec<PrimitiveSceneState> {
    let mut states = vec![PrimitiveSceneState::default(); scene.primitives.len()];
    if !scene.scene_nodes.is_empty() {
        assign_primitive_states(scene, SceneNodeId(0), [0.0, 0.0], 1.0, None, &mut states);
    }
    states
}

fn assign_primitive_states(
    scene: &CompiledScene,
    node_id: SceneNodeId,
    parent_offset: [f32; 2],
    parent_opacity: f32,
    parent_clip: Option<crate::scene::LayoutBox>,
    states: &mut [PrimitiveSceneState],
) {
    let node = &scene.scene_nodes[node_id.0 as usize];
    let current_offset = [
        parent_offset[0] + node.transform.tx,
        parent_offset[1] + node.transform.ty,
    ];
    let current_opacity = parent_opacity * node.opacity;
    let local_clip = node.clip.bounds.map(|bounds| crate::scene::LayoutBox {
        x: bounds.x + current_offset[0],
        y: bounds.y + current_offset[1],
        width: bounds.width,
        height: bounds.height,
    });
    let current_clip = intersect_clip(parent_clip, local_clip);

    for primitive_index in node.primitive_range.as_range() {
        states[primitive_index] = PrimitiveSceneState {
            offset: current_offset,
            opacity: current_opacity,
            clip: current_clip,
        };
    }

    let mut child = node.first_child;
    while let Some(child_id) = child {
        assign_primitive_states(
            scene,
            child_id,
            current_offset,
            current_opacity,
            current_clip,
            states,
        );
        child = scene.scene_nodes[child_id.0 as usize].next_sibling;
    }
}

fn clip_rect(
    rect: crate::scene::LayoutBox,
    clip: Option<crate::scene::LayoutBox>,
) -> Option<crate::scene::LayoutBox> {
    clip.map_or(Some(rect), |clip| intersect_rect(rect, clip))
}

fn clip_text_glyph(
    rect: crate::scene::LayoutBox,
    uv: crate::scene::LayoutBox,
    clip: Option<crate::scene::LayoutBox>,
) -> Option<(crate::scene::LayoutBox, crate::scene::LayoutBox)> {
    let clipped = clip_rect(rect, clip)?;
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }

    let left_ratio = (clipped.x - rect.x) / rect.width;
    let top_ratio = (clipped.y - rect.y) / rect.height;
    let right_ratio = (clipped.x + clipped.width - rect.x) / rect.width;
    let bottom_ratio = (clipped.y + clipped.height - rect.y) / rect.height;

    let clipped_uv = crate::scene::LayoutBox {
        x: uv.x + uv.width * left_ratio,
        y: uv.y + uv.height * top_ratio,
        width: uv.width * (right_ratio - left_ratio),
        height: uv.height * (bottom_ratio - top_ratio),
    };

    Some((clipped, clipped_uv))
}

fn intersect_clip(
    a: Option<crate::scene::LayoutBox>,
    b: Option<crate::scene::LayoutBox>,
) -> Option<crate::scene::LayoutBox> {
    match (a, b) {
        (Some(a), Some(b)) => intersect_rect(a, b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn intersect_rect(
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

fn create_quad_pipeline(
    device: &Device,
    view_layout: &BindGroupLayout,
    surface_format: TextureFormat,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("nekoui_quad_shader"),
        source: ShaderSource::Wgsl(QUAD_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("nekoui_quad_pipeline_layout"),
        bind_group_layouts: &[Some(view_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("nekoui_quad_pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[VertexBufferLayout {
                array_stride: std::mem::size_of::<QuadInstance>() as u64,
                step_mode: VertexStepMode::Instance,
                attributes: &vertex_attr_array![0 => Float32x4, 1 => Float32x4],
            }],
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_text_texture_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("nekoui_text_texture_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
        ],
    })
}

fn create_mono_text_pipeline(
    device: &Device,
    view_layout: &BindGroupLayout,
    glyph_layout: &BindGroupLayout,
    surface_format: TextureFormat,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("nekoui_text_shader"),
        source: ShaderSource::Wgsl(TEXT_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("nekoui_mono_text_pipeline_layout"),
        bind_group_layouts: &[Some(view_layout), Some(glyph_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("nekoui_mono_text_pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_mono"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[VertexBufferLayout {
                array_stride: std::mem::size_of::<TextInstance>() as u64,
                step_mode: VertexStepMode::Instance,
                attributes: &vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
            }],
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_mono"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_color_text_pipeline(
    device: &Device,
    view_layout: &BindGroupLayout,
    glyph_layout: &BindGroupLayout,
    surface_format: TextureFormat,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("nekoui_text_shader"),
        source: ShaderSource::Wgsl(TEXT_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("nekoui_color_text_pipeline_layout"),
        bind_group_layouts: &[Some(view_layout), Some(glyph_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("nekoui_color_text_pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_color"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[VertexBufferLayout {
                array_stride: std::mem::size_of::<ColorTextInstance>() as u64,
                step_mode: VertexStepMode::Instance,
                attributes: &vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32],
            }],
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_color"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_instance_buffer<T: Pod>(device: &Device, label: &str, capacity: usize) -> Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (std::mem::size_of::<T>() * capacity.max(1)) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
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
