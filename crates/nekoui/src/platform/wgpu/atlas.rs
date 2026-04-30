use cosmic_text::CacheKey;
use etagere::{Allocation, AtlasAllocator, size2};
use rustc_hash::FxHashMap;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, Device, FilterMode, Origin3d,
    Queue, Sampler, SamplerDescriptor, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
    TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor,
};

use crate::error::PlatformError;

const GLYPH_ATLAS_BYTE_BUDGET: u64 = 64 * 1024 * 1024;
const MAX_ATLAS_PAGES_PER_FAMILY: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlyphAtlasKind {
    Mono,
    Color,
}

pub(crate) struct GlyphAtlas {
    kind: GlyphAtlasKind,
    bind_group_layout: BindGroupLayout,
    width: u32,
    height: u32,
    pages: Vec<AtlasPage>,
    entries: FxHashMap<CacheKey, AtlasEntry>,
    next_page_id: u32,
    frame_id: u64,
    generation: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct AtlasEntry {
    pub(crate) page_id: u32,
    _allocation: Allocation,
    pub(crate) placement_left: i32,
    pub(crate) placement_top: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) uv_rect: [f32; 4],
}

struct AtlasPage {
    id: u32,
    allocator: AtlasAllocator,
    texture: Texture,
    _view: TextureView,
    _sampler: Sampler,
    bind_group: BindGroup,
    entries: FxHashMap<CacheKey, AtlasEntry>,
    used_in_frame: bool,
    last_used_frame: u64,
}

struct UploadPayload {
    placement_left: i32,
    placement_top: i32,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
    bytes_per_row: u32,
    padding: u32,
}

pub(crate) struct GlyphBitmapUpload<'a> {
    pub(crate) placement_left: i32,
    pub(crate) placement_top: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bytes: &'a [u8],
}

impl GlyphAtlas {
    pub(crate) fn new(
        _device: &Device,
        bind_group_layout: &BindGroupLayout,
        kind: GlyphAtlasKind,
        size: u32,
    ) -> Result<Self, PlatformError> {
        let size = size.max(1).min(u16::MAX as u32);
        let atlas = Self {
            kind,
            bind_group_layout: bind_group_layout.clone(),
            width: size,
            height: size,
            pages: Vec::new(),
            entries: FxHashMap::default(),
            next_page_id: 0,
            frame_id: 0,
            generation: 0,
        };
        Ok(atlas)
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame_id = self.frame_id.saturating_add(1);
        for page in &mut self.pages {
            page.used_in_frame = false;
        }
    }

    pub(crate) fn bind_group(&mut self, page_id: u32) -> Option<&BindGroup> {
        self.mark_page_used(page_id);
        self.pages
            .iter()
            .find(|page| page.id == page_id)
            .map(|page| &page.bind_group)
    }

    pub(crate) fn get(&mut self, key: &CacheKey) -> Option<AtlasEntry> {
        let entry = self.entries.get(key).copied()?;
        self.mark_page_used(entry.page_id);
        Some(entry)
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn upload_mask_bytes(
        &mut self,
        device: &Device,
        queue: &Queue,
        key: CacheKey,
        upload: GlyphBitmapUpload<'_>,
    ) -> Option<AtlasEntry> {
        debug_assert_eq!(self.kind, GlyphAtlasKind::Mono);
        self.upload_impl(
            device,
            queue,
            key,
            UploadPayload {
                placement_left: upload.placement_left,
                placement_top: upload.placement_top,
                width: upload.width,
                height: upload.height,
                bytes: upload.bytes.to_vec(),
                bytes_per_row: upload.width,
                padding: 0,
            },
        )
    }

    pub(crate) fn upload_color_bytes(
        &mut self,
        device: &Device,
        queue: &Queue,
        key: CacheKey,
        upload: GlyphBitmapUpload<'_>,
    ) -> Option<AtlasEntry> {
        debug_assert_eq!(self.kind, GlyphAtlasKind::Color);
        let padding = 1;
        let padded_width = upload.width + padding * 2;
        self.upload_impl(
            device,
            queue,
            key,
            UploadPayload {
                placement_left: upload.placement_left,
                placement_top: upload.placement_top,
                width: upload.width,
                height: upload.height,
                bytes: pad_rgba_with_border(upload.bytes, upload.width, upload.height, padding),
                bytes_per_row: padded_width * 4,
                padding,
            },
        )
    }

    fn upload_impl(
        &mut self,
        device: &Device,
        queue: &Queue,
        key: CacheKey,
        payload: UploadPayload,
    ) -> Option<AtlasEntry> {
        if let Some(entry) = self.entries.get(&key).copied() {
            self.mark_page_used(entry.page_id);
            return Some(entry);
        }

        if payload.width == 0 || payload.height == 0 {
            return None;
        }

        let padded_width = payload.width + payload.padding * 2;
        let padded_height = payload.height + payload.padding * 2;
        let size = size2(padded_width as i32, padded_height as i32);
        let (page_index, allocation) = self.allocate_page_region(device, size)?;
        let page_id = self.pages[page_index].id;

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.pages[page_index].texture,
                mip_level: 0,
                origin: Origin3d {
                    x: allocation.rectangle.min.x as u32,
                    y: allocation.rectangle.min.y as u32,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &payload.bytes,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(payload.bytes_per_row),
                rows_per_image: Some(padded_height),
            },
            wgpu::Extent3d {
                width: padded_width,
                height: padded_height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_width = self.width as f32;
        let atlas_height = self.height as f32;
        let uv_origin_x = allocation.rectangle.min.x as f32 + payload.padding as f32;
        let uv_origin_y = allocation.rectangle.min.y as f32 + payload.padding as f32;
        let entry = AtlasEntry {
            page_id,
            _allocation: allocation,
            placement_left: payload.placement_left,
            placement_top: payload.placement_top,
            width: payload.width,
            height: payload.height,
            uv_rect: [
                uv_origin_x / atlas_width,
                uv_origin_y / atlas_height,
                payload.width as f32 / atlas_width,
                payload.height as f32 / atlas_height,
            ],
        };
        self.pages[page_index].entries.insert(key, entry);
        self.pages[page_index].used_in_frame = true;
        self.pages[page_index].last_used_frame = self.frame_id;
        self.entries.insert(key, entry);
        self.generation = self.generation.saturating_add(1);
        Some(entry)
    }

    fn create_page(&mut self, device: &Device) -> AtlasPage {
        let page_id = self.next_page_id;
        self.next_page_id += 1;

        let texture = device.create_texture(&TextureDescriptor {
            label: Some(match self.kind {
                GlyphAtlasKind::Mono => "nekoui_mono_glyph_atlas",
                GlyphAtlasKind::Color => "nekoui_color_glyph_atlas",
            }),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: match self.kind {
                GlyphAtlasKind::Mono => TextureFormat::R8Unorm,
                GlyphAtlasKind::Color => TextureFormat::Rgba8UnormSrgb,
            },
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
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some(match self.kind {
                GlyphAtlasKind::Mono => "nekoui_mono_glyph_bind_group",
                GlyphAtlasKind::Color => "nekoui_color_glyph_bind_group",
            }),
            layout: &self.bind_group_layout,
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

        AtlasPage {
            id: page_id,
            allocator: AtlasAllocator::new(size2(self.width as i32, self.height as i32)),
            texture,
            _view: view,
            _sampler: sampler,
            bind_group,
            entries: FxHashMap::default(),
            used_in_frame: false,
            last_used_frame: self.frame_id,
        }
    }

    fn allocate_page_region(
        &mut self,
        device: &Device,
        size: etagere::Size,
    ) -> Option<(usize, Allocation)> {
        if self.pages.is_empty() {
            let page = self.create_page(device);
            self.pages.push(page);
        }

        let current_index = self.pages.len() - 1;
        if let Some(allocation) = self.pages[current_index].allocator.allocate(size) {
            return Some((current_index, allocation));
        }

        if self.pages.len() < self.max_pages() {
            let page = self.create_page(device);
            self.pages.push(page);
            let current_index = self.pages.len() - 1;
            return self.pages[current_index]
                .allocator
                .allocate(size)
                .map(|allocation| (current_index, allocation));
        }

        self.evict_unused_pages();

        if self.pages.is_empty() {
            let page = self.create_page(device);
            self.pages.push(page);
            return self.pages[0]
                .allocator
                .allocate(size)
                .map(|allocation| (0, allocation));
        }

        let current_index = self.pages.len() - 1;
        if let Some(allocation) = self.pages[current_index].allocator.allocate(size) {
            return Some((current_index, allocation));
        }

        if self.pages.len() < self.max_pages() {
            let page = self.create_page(device);
            self.pages.push(page);
            let current_index = self.pages.len() - 1;
            return self.pages[current_index]
                .allocator
                .allocate(size)
                .map(|allocation| (current_index, allocation));
        }

        None
    }

    fn evict_unused_pages(&mut self) {
        let max_pages = self.max_pages();
        if self.pages.len() < max_pages {
            return;
        }

        let eviction_ids = eviction_page_ids(
            &self
                .pages
                .iter()
                .map(|page| AtlasPageState {
                    id: page.id,
                    used_in_frame: page.used_in_frame,
                    last_used_frame: page.last_used_frame,
                })
                .collect::<Vec<_>>(),
            max_pages,
        );

        if eviction_ids.is_empty() {
            return;
        }

        for page_id in &eviction_ids {
            if let Some(page) = self.pages.iter().find(|page| page.id == *page_id) {
                for key in page.entries.keys().copied().collect::<Vec<_>>() {
                    self.entries.remove(&key);
                }
            }
        }

        self.pages.retain(|page| !eviction_ids.contains(&page.id));
        self.generation = self.generation.saturating_add(1);
    }

    fn mark_page_used(&mut self, page_id: u32) {
        if let Some(page) = self.pages.iter_mut().find(|page| page.id == page_id) {
            page.used_in_frame = true;
            page.last_used_frame = self.frame_id;
        }
    }

    fn max_pages(&self) -> usize {
        let budget_pages = (GLYPH_ATLAS_BYTE_BUDGET / self.page_byte_size()).max(1) as usize;
        budget_pages.min(MAX_ATLAS_PAGES_PER_FAMILY)
    }

    fn page_byte_size(&self) -> u64 {
        let bytes_per_pixel = match self.kind {
            GlyphAtlasKind::Mono => 1_u64,
            GlyphAtlasKind::Color => 4_u64,
        };
        u64::from(self.width) * u64::from(self.height) * bytes_per_pixel
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtlasPageState {
    id: u32,
    used_in_frame: bool,
    last_used_frame: u64,
}

fn eviction_page_ids(pages: &[AtlasPageState], max_pages: usize) -> Vec<u32> {
    if pages.len() < max_pages {
        return Vec::new();
    }

    let removable = pages.len().saturating_sub(max_pages - 1);
    let mut candidates = pages
        .iter()
        .copied()
        .filter(|page| !page.used_in_frame)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|page| page.last_used_frame);
    candidates
        .into_iter()
        .take(removable)
        .map(|page| page.id)
        .collect()
}

fn pad_rgba_with_border(bytes: &[u8], width: u32, height: u32, padding: u32) -> Vec<u8> {
    let padded_width = width + padding * 2;
    let padded_height = height + padding * 2;
    let mut out = vec![0_u8; (padded_width * padded_height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let src_index = ((y * width + x) * 4) as usize;
            let dst_x = x + padding;
            let dst_y = y + padding;
            let dst_index = ((dst_y * padded_width + dst_x) * 4) as usize;
            out[dst_index..dst_index + 4].copy_from_slice(&bytes[src_index..src_index + 4]);
        }
    }

    if padding == 0 || width == 0 || height == 0 {
        return out;
    }

    for y in padding..(padding + height) {
        let row_start = (y * padded_width * 4) as usize;
        let first = row_start + (padding * 4) as usize;
        let last = row_start + ((padding + width - 1) * 4) as usize;
        let first_pixel = [out[first], out[first + 1], out[first + 2], out[first + 3]];
        let last_pixel = [out[last], out[last + 1], out[last + 2], out[last + 3]];
        for px in 0..padding {
            let left = row_start + (px * 4) as usize;
            out[left..left + 4].copy_from_slice(&first_pixel);
            let right = row_start + ((padding + width + px) * 4) as usize;
            out[right..right + 4].copy_from_slice(&last_pixel);
        }
    }

    for px_y in 0..padding {
        let src_top = (padding * padded_width * 4) as usize;
        let dst_top = (px_y * padded_width * 4) as usize;
        out.copy_within(src_top..src_top + (padded_width * 4) as usize, dst_top);

        let src_bottom = ((padding + height - 1) * padded_width * 4) as usize;
        let dst_bottom = ((padding + height + px_y) * padded_width * 4) as usize;
        out.copy_within(
            src_bottom..src_bottom + (padded_width * 4) as usize,
            dst_bottom,
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{AtlasPageState, GLYPH_ATLAS_BYTE_BUDGET, eviction_page_ids, pad_rgba_with_border};

    #[test]
    fn eviction_prefers_oldest_unused_pages() {
        let eviction = eviction_page_ids(
            &[
                AtlasPageState {
                    id: 1,
                    used_in_frame: true,
                    last_used_frame: 9,
                },
                AtlasPageState {
                    id: 2,
                    used_in_frame: false,
                    last_used_frame: 3,
                },
                AtlasPageState {
                    id: 3,
                    used_in_frame: false,
                    last_used_frame: 7,
                },
                AtlasPageState {
                    id: 4,
                    used_in_frame: false,
                    last_used_frame: 5,
                },
            ],
            4,
        );

        assert_eq!(eviction, vec![2]);
    }

    #[test]
    fn eviction_keeps_used_pages_even_when_full() {
        let eviction = eviction_page_ids(
            &[
                AtlasPageState {
                    id: 1,
                    used_in_frame: true,
                    last_used_frame: 9,
                },
                AtlasPageState {
                    id: 2,
                    used_in_frame: true,
                    last_used_frame: 8,
                },
                AtlasPageState {
                    id: 3,
                    used_in_frame: true,
                    last_used_frame: 7,
                },
                AtlasPageState {
                    id: 4,
                    used_in_frame: true,
                    last_used_frame: 6,
                },
            ],
            4,
        );

        assert!(eviction.is_empty());
    }

    #[test]
    fn color_family_page_budget_caps_pages_more_aggressively() {
        let page_bytes = 2048_u64 * 2048_u64 * 4_u64;
        let budget_pages = (GLYPH_ATLAS_BYTE_BUDGET / page_bytes).max(1) as usize;
        assert_eq!(page_bytes, 16 * 1024 * 1024);
        assert_eq!(budget_pages.min(16), 4);
    }

    #[test]
    fn color_upload_copies_edge_padding() {
        let bytes = vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
        let out = pad_rgba_with_border(&bytes, 2, 1, 1);
        assert_eq!(out.len(), 4 * 4 * 3);
        assert_eq!(&out[0..4], &[0x10, 0x20, 0x30, 0x40]);
        assert_eq!(&out[4..8], &[0x10, 0x20, 0x30, 0x40]);
        assert_eq!(&out[8..12], &[0x50, 0x60, 0x70, 0x80]);
    }
}
