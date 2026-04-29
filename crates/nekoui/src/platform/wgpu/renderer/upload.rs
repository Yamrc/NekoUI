use bytemuck::Pod;
use wgpu::util::StagingBelt;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, Buffer, BufferSize, Device,
};

use super::{
    RenderSystem, SHRINK_IDLE_FRAME_THRESHOLD,
    types::{ClipSlotInstance, ColorTextInstance, RectInstance, TextInstance},
};

pub(super) fn stage_write_pod_slice<T: Pod>(
    staging_belt: &mut StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
    target: &Buffer,
    values: &[T],
) {
    if values.is_empty() {
        return;
    }
    stage_write_bytes(staging_belt, encoder, target, bytemuck::cast_slice(values));
}

pub(super) fn stage_write_bytes(
    staging_belt: &mut StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
    target: &Buffer,
    bytes: &[u8],
) {
    if bytes.is_empty() {
        return;
    }

    let aligned_size = align_copy_size(bytes.len() as u64);
    let mut view = staging_belt.write_buffer(
        encoder,
        target,
        0,
        BufferSize::new(aligned_size).expect("aligned size must be non-zero"),
    );
    debug_assert_eq!(aligned_size as usize, bytes.len());
    view.copy_from_slice(bytes);
}

fn align_copy_size(size: u64) -> u64 {
    size.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
}

pub(super) fn create_storage_buffer<T: Pod>(
    device: &Device,
    label: &str,
    capacity: usize,
) -> Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (std::mem::size_of::<T>() * capacity.max(1)) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(super) fn create_rect_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    rect_buffer: &Buffer,
    clip_buffer: &Buffer,
) -> BindGroup {
    create_instance_clip_bind_group(
        device,
        layout,
        rect_buffer,
        clip_buffer,
        "nekoui_rect_bind_group",
    )
}

pub(super) fn create_text_instance_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    instance_buffer: &Buffer,
    clip_buffer: &Buffer,
    label: &'static str,
) -> BindGroup {
    create_instance_clip_bind_group(device, layout, instance_buffer, clip_buffer, label)
}

fn create_instance_clip_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    instance_buffer: &Buffer,
    clip_buffer: &Buffer,
    label: &'static str,
) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: instance_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: clip_buffer.as_entire_binding(),
            },
        ],
    })
}

struct StorageBufferResizeSlot<'a> {
    min_capacity: usize,
    capacity: &'a mut usize,
    low_usage_frames: &'a mut u32,
    buffer: &'a mut Buffer,
    bind_group: &'a mut BindGroup,
    layout: &'a BindGroupLayout,
    buffer_label: &'static str,
    bind_group_label: &'static str,
}

struct ClipStorageResizeSlot<'a> {
    min_capacity: usize,
    capacity: &'a mut usize,
    low_usage_frames: &'a mut u32,
    buffer: &'a mut Buffer,
    buffer_label: &'static str,
}

fn maybe_shrink_storage_buffer<T: Pod>(
    device: &Device,
    count: usize,
    slot: StorageBufferResizeSlot<'_>,
) -> bool {
    if *slot.capacity <= slot.min_capacity {
        *slot.low_usage_frames = 0;
        return false;
    }

    if count.saturating_mul(4) > *slot.capacity {
        *slot.low_usage_frames = 0;
        return false;
    }

    *slot.low_usage_frames += 1;
    if *slot.low_usage_frames < SHRINK_IDLE_FRAME_THRESHOLD {
        return false;
    }

    let target = count
        .max(1)
        .saturating_mul(2)
        .max(slot.min_capacity)
        .next_power_of_two();
    if target >= *slot.capacity {
        *slot.low_usage_frames = 0;
        return false;
    }

    *slot.capacity = target;
    let buffer = create_storage_buffer::<T>(device, slot.buffer_label, *slot.capacity);
    *slot.buffer = buffer;
    *slot.bind_group =
        create_single_storage_bind_group(device, slot.layout, slot.buffer, slot.bind_group_label);
    *slot.low_usage_frames = 0;
    true
}

fn grow_capacity(capacity: &mut usize, count: usize) -> bool {
    if count <= *capacity {
        return false;
    }

    while *capacity < count {
        *capacity *= 2;
    }
    true
}

fn maybe_shrink_clip_storage_buffer<T: Pod>(
    device: &Device,
    count: usize,
    slot: ClipStorageResizeSlot<'_>,
) -> bool {
    if *slot.capacity <= slot.min_capacity {
        *slot.low_usage_frames = 0;
        return false;
    }

    if count.saturating_mul(4) > *slot.capacity {
        *slot.low_usage_frames = 0;
        return false;
    }

    *slot.low_usage_frames += 1;
    if *slot.low_usage_frames < SHRINK_IDLE_FRAME_THRESHOLD {
        return false;
    }

    let target = count
        .max(1)
        .saturating_mul(2)
        .max(slot.min_capacity)
        .next_power_of_two();
    if target >= *slot.capacity {
        *slot.low_usage_frames = 0;
        return false;
    }

    *slot.capacity = target;
    *slot.buffer = create_storage_buffer::<T>(device, slot.buffer_label, *slot.capacity);
    *slot.low_usage_frames = 0;
    true
}

fn create_single_storage_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    buffer: &Buffer,
    label: &'static str,
) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

pub(super) fn rebuild_text_instance_storage<T: Pod>(
    device: &Device,
    layout: &BindGroupLayout,
    clip_buffer: &Buffer,
    capacity: usize,
    buffer_label: &str,
    bind_group_label: &'static str,
) -> (Buffer, BindGroup) {
    let buffer = create_storage_buffer::<T>(device, buffer_label, capacity);
    let bind_group =
        create_text_instance_bind_group(device, layout, &buffer, clip_buffer, bind_group_label);
    (buffer, bind_group)
}

pub(super) fn rebuild_rect_storage(
    device: &Device,
    layout: &BindGroupLayout,
    clip_buffer: &Buffer,
    capacity: usize,
) -> (Buffer, BindGroup) {
    let buffer = create_storage_buffer::<RectInstance>(device, "nekoui_rect_instances", capacity);
    let bind_group = create_rect_bind_group(device, layout, &buffer, clip_buffer);
    (buffer, bind_group)
}

impl RenderSystem {
    pub(super) fn ensure_rect_capacity(&mut self, count: usize) {
        if !grow_capacity(&mut self.rect_instance_capacity, count) {
            return;
        }
        let (buffer, bind_group) = rebuild_rect_storage(
            &self.context.device,
            &self.rect_bind_group_layout,
            &self.clip_slot_buffer,
            self.rect_instance_capacity,
        );
        self.rect_storage_buffer = buffer;
        self.rect_bind_group = bind_group;
        self.buffer_epoch = self.buffer_epoch.saturating_add(1);
    }

    pub(super) fn maybe_shrink_rect_capacity(&mut self, count: usize) {
        if maybe_shrink_storage_buffer::<RectInstance>(
            &self.context.device,
            count,
            StorageBufferResizeSlot {
                min_capacity: 64,
                capacity: &mut self.rect_instance_capacity,
                low_usage_frames: &mut self.rect_low_usage_frames,
                buffer: &mut self.rect_storage_buffer,
                bind_group: &mut self.rect_bind_group,
                layout: &self.rect_bind_group_layout,
                buffer_label: "nekoui_rect_instances",
                bind_group_label: "nekoui_rect_bind_group",
            },
        ) {
            self.rect_bind_group = create_rect_bind_group(
                &self.context.device,
                &self.rect_bind_group_layout,
                &self.rect_storage_buffer,
                &self.clip_slot_buffer,
            );
            self.buffer_epoch = self.buffer_epoch.saturating_add(1);
        }
    }

    pub(super) fn ensure_mono_text_capacity(&mut self, count: usize) {
        if !grow_capacity(&mut self.mono_text_instance_capacity, count) {
            return;
        }
        let (buffer, bind_group) = rebuild_text_instance_storage::<TextInstance>(
            &self.context.device,
            &self.text_instance_bind_group_layout,
            &self.clip_slot_buffer,
            self.mono_text_instance_capacity,
            "nekoui_mono_text_instances",
            "nekoui_mono_text_bind_group",
        );
        self.mono_text_instance_buffer = buffer;
        self.mono_text_bind_group = bind_group;
        self.buffer_epoch = self.buffer_epoch.saturating_add(1);
    }

    pub(super) fn maybe_shrink_mono_text_capacity(&mut self, count: usize) {
        if maybe_shrink_storage_buffer::<TextInstance>(
            &self.context.device,
            count,
            StorageBufferResizeSlot {
                min_capacity: 256,
                capacity: &mut self.mono_text_instance_capacity,
                low_usage_frames: &mut self.mono_text_low_usage_frames,
                buffer: &mut self.mono_text_instance_buffer,
                bind_group: &mut self.mono_text_bind_group,
                layout: &self.text_instance_bind_group_layout,
                buffer_label: "nekoui_mono_text_instances",
                bind_group_label: "nekoui_mono_text_bind_group",
            },
        ) {
            self.mono_text_bind_group = create_text_instance_bind_group(
                &self.context.device,
                &self.text_instance_bind_group_layout,
                &self.mono_text_instance_buffer,
                &self.clip_slot_buffer,
                "nekoui_mono_text_bind_group",
            );
            self.buffer_epoch = self.buffer_epoch.saturating_add(1);
        }
    }

    pub(super) fn ensure_color_text_capacity(&mut self, count: usize) {
        if !grow_capacity(&mut self.color_text_instance_capacity, count) {
            return;
        }
        let (buffer, bind_group) = rebuild_text_instance_storage::<ColorTextInstance>(
            &self.context.device,
            &self.text_instance_bind_group_layout,
            &self.clip_slot_buffer,
            self.color_text_instance_capacity,
            "nekoui_color_text_instances",
            "nekoui_color_text_bind_group",
        );
        self.color_text_instance_buffer = buffer;
        self.color_text_bind_group = bind_group;
        self.buffer_epoch = self.buffer_epoch.saturating_add(1);
    }

    pub(super) fn maybe_shrink_color_text_capacity(&mut self, count: usize) {
        if maybe_shrink_storage_buffer::<ColorTextInstance>(
            &self.context.device,
            count,
            StorageBufferResizeSlot {
                min_capacity: 64,
                capacity: &mut self.color_text_instance_capacity,
                low_usage_frames: &mut self.color_text_low_usage_frames,
                buffer: &mut self.color_text_instance_buffer,
                bind_group: &mut self.color_text_bind_group,
                layout: &self.text_instance_bind_group_layout,
                buffer_label: "nekoui_color_text_instances",
                bind_group_label: "nekoui_color_text_bind_group",
            },
        ) {
            self.color_text_bind_group = create_text_instance_bind_group(
                &self.context.device,
                &self.text_instance_bind_group_layout,
                &self.color_text_instance_buffer,
                &self.clip_slot_buffer,
                "nekoui_color_text_bind_group",
            );
            self.buffer_epoch = self.buffer_epoch.saturating_add(1);
        }
    }

    pub(super) fn ensure_clip_slot_capacity(&mut self, count: usize) {
        if !grow_capacity(&mut self.clip_slot_capacity, count) {
            return;
        }
        self.clip_slot_buffer = create_storage_buffer::<ClipSlotInstance>(
            &self.context.device,
            "nekoui_clip_slots",
            self.clip_slot_capacity,
        );
        self.refresh_clip_bind_groups();
        self.buffer_epoch = self.buffer_epoch.saturating_add(1);
    }

    pub(super) fn maybe_shrink_clip_slot_capacity(&mut self, count: usize) {
        if maybe_shrink_clip_storage_buffer::<ClipSlotInstance>(
            &self.context.device,
            count,
            ClipStorageResizeSlot {
                min_capacity: 64,
                capacity: &mut self.clip_slot_capacity,
                low_usage_frames: &mut self.clip_slot_low_usage_frames,
                buffer: &mut self.clip_slot_buffer,
                buffer_label: "nekoui_clip_slots",
            },
        ) {
            self.refresh_clip_bind_groups();
            self.buffer_epoch = self.buffer_epoch.saturating_add(1);
        }
    }

    fn refresh_clip_bind_groups(&mut self) {
        self.rect_bind_group = create_rect_bind_group(
            &self.context.device,
            &self.rect_bind_group_layout,
            &self.rect_storage_buffer,
            &self.clip_slot_buffer,
        );
        self.mono_text_bind_group = create_text_instance_bind_group(
            &self.context.device,
            &self.text_instance_bind_group_layout,
            &self.mono_text_instance_buffer,
            &self.clip_slot_buffer,
            "nekoui_mono_text_bind_group",
        );
        self.color_text_bind_group = create_text_instance_bind_group(
            &self.context.device,
            &self.text_instance_bind_group_layout,
            &self.color_text_instance_buffer,
            &self.clip_slot_buffer,
            "nekoui_color_text_bind_group",
        );
    }
}
