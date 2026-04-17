use cosmic_text::{CacheKey, SwashContent};
use etagere::{Allocation, AtlasAllocator, size2};
use hashbrown::HashMap;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Device, FilterMode, Origin3d, Queue, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderStages, TexelCopyBufferLayout,
    TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};

use crate::error::PlatformError;

pub(crate) struct GlyphAtlas {
    allocator: AtlasAllocator,
    texture: Texture,
    _view: TextureView,
    _sampler: Sampler,
    bind_group: BindGroup,
    bind_group_layout: BindGroupLayout,
    width: u32,
    height: u32,
    entries: HashMap<CacheKey, AtlasEntry>,
}

#[derive(Clone, Copy)]
pub(crate) struct AtlasEntry {
    _allocation: Allocation,
    pub(crate) placement_left: i32,
    pub(crate) placement_top: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) uv_rect: [f32; 4],
}

impl GlyphAtlas {
    pub(crate) fn new(device: &Device, size: u32) -> Result<Self, PlatformError> {
        let size = size.max(1).min(u16::MAX as u32);
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("nekoui_glyph_atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("nekoui_glyph_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("nekoui_glyph_bind_group_layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("nekoui_glyph_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });

        Ok(Self {
            allocator: AtlasAllocator::new(size2(size as i32, size as i32)),
            texture,
            _view: view,
            _sampler: sampler,
            bind_group,
            bind_group_layout,
            width: size,
            height: size,
            entries: HashMap::new(),
        })
    }

    pub(crate) fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    pub(crate) fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.bind_group_layout
    }

    pub(crate) fn get(&self, key: &CacheKey) -> Option<AtlasEntry> {
        self.entries.get(key).copied()
    }

    pub(crate) fn upload(
        &mut self,
        queue: &Queue,
        key: CacheKey,
        image: &cosmic_text::SwashImage,
    ) -> Option<AtlasEntry> {
        if let Some(entry) = self.entries.get(&key).copied() {
            return Some(entry);
        }

        if image.placement.width == 0 || image.placement.height == 0 {
            return None;
        }

        let size = size2(image.placement.width as i32, image.placement.height as i32);
        let allocation = self.allocator.allocate(size).or_else(|| {
            self.allocator.clear();
            self.entries.clear();
            self.allocator.allocate(size)
        })?;

        let rgba = image_to_rgba(image);
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: allocation.rectangle.min.x as u32,
                    y: allocation.rectangle.min.y as u32,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &rgba,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.placement.width * 4),
                rows_per_image: Some(image.placement.height),
            },
            wgpu::Extent3d {
                width: image.placement.width,
                height: image.placement.height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_width = self.width as f32;
        let atlas_height = self.height as f32;
        let entry = AtlasEntry {
            _allocation: allocation,
            placement_left: image.placement.left,
            placement_top: image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            uv_rect: [
                allocation.rectangle.min.x as f32 / atlas_width,
                allocation.rectangle.min.y as f32 / atlas_height,
                image.placement.width as f32 / atlas_width,
                image.placement.height as f32 / atlas_height,
            ],
        };
        self.entries.insert(key, entry);
        Some(entry)
    }
}

fn image_to_rgba(image: &cosmic_text::SwashImage) -> Vec<u8> {
    let pixel_count = (image.placement.width * image.placement.height) as usize;
    match image.content {
        SwashContent::Mask => {
            let mut rgba = vec![0_u8; pixel_count * 4];
            for (index, alpha) in image.data.iter().copied().enumerate() {
                let base = index * 4;
                rgba[base] = 255;
                rgba[base + 1] = 255;
                rgba[base + 2] = 255;
                rgba[base + 3] = alpha;
            }
            rgba
        }
        SwashContent::Color | SwashContent::SubpixelMask => image.data.clone(),
    }
}
