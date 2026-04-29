use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
    BufferBindingType, ColorTargetState, ColorWrites, Device, FragmentState, MultisampleState,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, TextureFormat,
    VertexState,
};

use crate::platform::wgpu::shader::{RECT_SHADER, TEXT_SHADER};

use super::RenderSystem;

impl RenderSystem {
    pub(super) fn ensure_pipelines_for_format(&mut self, surface_format: TextureFormat) {
        if self.current_surface_format == Some(surface_format) {
            return;
        }
        self.rect_pipeline = create_rect_pipeline(
            &self.context.device,
            &self.view_bind_group_layout,
            &self.rect_bind_group_layout,
            surface_format,
            self.pipeline_cache.as_ref(),
        );
        self.mono_text_pipeline = create_mono_text_pipeline(
            &self.context.device,
            &self.view_bind_group_layout,
            &self.text_texture_bind_group_layout,
            &self.text_instance_bind_group_layout,
            surface_format,
            self.pipeline_cache.as_ref(),
        );
        self.color_text_pipeline = create_color_text_pipeline(
            &self.context.device,
            &self.view_bind_group_layout,
            &self.text_texture_bind_group_layout,
            &self.text_instance_bind_group_layout,
            surface_format,
            self.pipeline_cache.as_ref(),
        );
        self.current_surface_format = Some(surface_format);
    }
}

pub(super) fn create_rect_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("nekoui_rect_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

pub(super) fn create_rect_pipeline(
    device: &Device,
    view_layout: &BindGroupLayout,
    rect_layout: &BindGroupLayout,
    surface_format: TextureFormat,
    pipeline_cache: Option<&wgpu::PipelineCache>,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("nekoui_rect_shader"),
        source: ShaderSource::Wgsl(RECT_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("nekoui_rect_pipeline_layout"),
        bind_group_layouts: &[Some(view_layout), Some(rect_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("nekoui_rect_pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[],
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
        cache: pipeline_cache,
    })
}

pub(super) fn create_text_instance_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("nekoui_text_instance_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

pub(super) fn create_text_texture_bind_group_layout(device: &Device) -> BindGroupLayout {
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

pub(super) fn create_mono_text_pipeline(
    device: &Device,
    view_layout: &BindGroupLayout,
    glyph_layout: &BindGroupLayout,
    text_instance_layout: &BindGroupLayout,
    surface_format: TextureFormat,
    pipeline_cache: Option<&wgpu::PipelineCache>,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("nekoui_text_shader"),
        source: ShaderSource::Wgsl(TEXT_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("nekoui_mono_text_pipeline_layout"),
        bind_group_layouts: &[
            Some(view_layout),
            Some(glyph_layout),
            Some(text_instance_layout),
        ],
        immediate_size: 0,
    });
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("nekoui_mono_text_pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_mono"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[],
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
        cache: pipeline_cache,
    })
}

pub(super) fn create_color_text_pipeline(
    device: &Device,
    view_layout: &BindGroupLayout,
    glyph_layout: &BindGroupLayout,
    text_instance_layout: &BindGroupLayout,
    surface_format: TextureFormat,
    pipeline_cache: Option<&wgpu::PipelineCache>,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("nekoui_text_shader"),
        source: ShaderSource::Wgsl(TEXT_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("nekoui_color_text_pipeline_layout"),
        bind_group_layouts: &[
            Some(view_layout),
            Some(glyph_layout),
            Some(text_instance_layout),
        ],
        immediate_size: 0,
    });
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("nekoui_color_text_pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_color"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[],
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
        cache: pipeline_cache,
    })
}
