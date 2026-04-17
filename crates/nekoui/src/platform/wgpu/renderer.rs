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
use crate::platform::wgpu::atlas::{AtlasEntry, GlyphAtlas};
use crate::platform::wgpu::context::WgpuContext;
use crate::scene::{CompiledScene, Primitive};
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
    text_pipeline: RenderPipeline,
    atlas: GlyphAtlas,
    swash_cache: SwashCache,
    quad_instances: Vec<QuadInstance>,
    text_instances: Vec<TextInstance>,
    quad_instance_buffer: Buffer,
    text_instance_buffer: Buffer,
    quad_instance_capacity: usize,
    text_instance_capacity: usize,
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

        let atlas = GlyphAtlas::new(&context.device, ATLAS_SIZE.min(context.max_texture_size))?;
        let quad_pipeline = create_quad_pipeline(
            &context.device,
            &view_bind_group_layout,
            TextureFormat::Bgra8UnormSrgb,
        );
        let text_pipeline = create_text_pipeline(
            &context.device,
            &view_bind_group_layout,
            atlas.bind_group_layout(),
            TextureFormat::Bgra8UnormSrgb,
        );

        let quad_instance_capacity = 64;
        let text_instance_capacity = 256;
        let quad_instance_buffer = create_instance_buffer::<QuadInstance>(
            &context.device,
            "nekoui_quad_instances",
            quad_instance_capacity,
        );
        let text_instance_buffer = create_instance_buffer::<TextInstance>(
            &context.device,
            "nekoui_text_instances",
            text_instance_capacity,
        );

        let mut render_system = Self {
            context,
            view_buffer,
            view_bind_group_layout,
            view_bind_group,
            quad_pipeline,
            text_pipeline,
            atlas,
            swash_cache: SwashCache::new(),
            quad_instances: Vec::new(),
            text_instances: Vec::new(),
            quad_instance_buffer,
            text_instance_buffer,
            quad_instance_capacity,
            text_instance_capacity,
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
        self.text_instances.clear();
        self.collect_instances(scene, text_system, scale_factor as f32);
        self.ensure_quad_capacity(self.quad_instances.len());
        self.ensure_text_capacity(self.text_instances.len());

        if !self.quad_instances.is_empty() {
            self.context.queue.write_buffer(
                &self.quad_instance_buffer,
                0,
                bytemuck::cast_slice(&self.quad_instances),
            );
        }
        if !self.text_instances.is_empty() {
            self.context.queue.write_buffer(
                &self.text_instance_buffer,
                0,
                bytemuck::cast_slice(&self.text_instances),
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

            if !self.text_instances.is_empty() {
                pass.set_pipeline(&self.text_pipeline);
                pass.set_bind_group(0, &self.view_bind_group, &[]);
                pass.set_bind_group(1, self.atlas.bind_group(), &[]);
                pass.set_vertex_buffer(0, self.text_instance_buffer.slice(..));
                pass.draw(0..6, 0..self.text_instances.len() as u32);
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
        for primitive in &scene.primitives {
            match primitive {
                Primitive::Quad { bounds, color } => self.quad_instances.push(QuadInstance {
                    rect: [
                        bounds.x * scale_factor,
                        bounds.y * scale_factor,
                        bounds.width * scale_factor,
                        bounds.height * scale_factor,
                    ],
                    color: [color.r, color.g, color.b, color.a],
                }),
                Primitive::Text {
                    bounds,
                    layout,
                    color,
                } => {
                    for run in &layout.runs {
                        for glyph in &run.glyphs {
                            let physical = glyph.physical(
                                (
                                    bounds.x * scale_factor,
                                    (bounds.y + run.baseline) * scale_factor,
                                ),
                                scale_factor,
                            );
                            let Some(entry) =
                                self.ensure_glyph_entry(text_system, physical.cache_key)
                            else {
                                continue;
                            };
                            let glyph_color =
                                glyph.color_opt.map(cosmic_to_style_color).unwrap_or(*color);
                            self.text_instances.push(TextInstance {
                                rect: [
                                    (physical.x + entry.placement_left) as f32,
                                    (physical.y - entry.placement_top) as f32,
                                    entry.width as f32,
                                    entry.height as f32,
                                ],
                                uv_rect: entry.uv_rect,
                                color: [glyph_color.r, glyph_color.g, glyph_color.b, glyph_color.a],
                            });
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
    ) -> Option<AtlasEntry> {
        if let Some(entry) = self.atlas.get(&cache_key) {
            return Some(entry);
        }
        let image = self
            .swash_cache
            .get_image(text_system.font_system_mut(), cache_key)
            .as_ref()?
            .clone();
        self.atlas.upload(&self.context.queue, cache_key, &image)
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
        self.text_pipeline = create_text_pipeline(
            &self.context.device,
            &self.view_bind_group_layout,
            self.atlas.bind_group_layout(),
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

    fn ensure_text_capacity(&mut self, count: usize) {
        if count <= self.text_instance_capacity {
            return;
        }
        while self.text_instance_capacity < count {
            self.text_instance_capacity *= 2;
        }
        self.text_instance_buffer = create_instance_buffer::<TextInstance>(
            &self.context.device,
            "nekoui_text_instances",
            self.text_instance_capacity,
        );
    }
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

fn create_text_pipeline(
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
        label: Some("nekoui_text_pipeline_layout"),
        bind_group_layouts: &[Some(view_layout), Some(glyph_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("nekoui_text_pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[VertexBufferLayout {
                array_stride: std::mem::size_of::<TextInstance>() as u64,
                step_mode: VertexStepMode::Instance,
                attributes: &vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
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
