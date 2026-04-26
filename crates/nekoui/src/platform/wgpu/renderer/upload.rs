use bytemuck::Pod;
use wgpu::util::StagingBelt;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, Buffer, BufferSize, Device,
};

use super::{
    RenderSystem, SHRINK_IDLE_FRAME_THRESHOLD,
    types::{ColorTextInstance, RectInstance, TextInstance},
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

pub(super) fn create_storage_bind_group(
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

pub(super) fn create_rect_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    buffer: &Buffer,
) -> BindGroup {
    create_storage_bind_group(device, layout, buffer, "nekoui_rect_bind_group")
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
    let (new_buffer, new_bind_group) = rebuild_storage::<T>(
        device,
        slot.layout,
        *slot.capacity,
        slot.buffer_label,
        slot.bind_group_label,
    );
    *slot.buffer = new_buffer;
    *slot.bind_group = new_bind_group;
    *slot.low_usage_frames = 0;
    true
}

pub(super) fn rebuild_storage<T: Pod>(
    device: &Device,
    layout: &BindGroupLayout,
    capacity: usize,
    buffer_label: &str,
    bind_group_label: &'static str,
) -> (Buffer, BindGroup) {
    let buffer = create_storage_buffer::<T>(device, buffer_label, capacity);
    let bind_group = create_storage_bind_group(device, layout, &buffer, bind_group_label);
    (buffer, bind_group)
}

pub(super) fn rebuild_rect_storage(
    device: &Device,
    layout: &BindGroupLayout,
    capacity: usize,
) -> (Buffer, BindGroup) {
    let (buffer, bind_group) = rebuild_storage::<RectInstance>(
        device,
        layout,
        capacity,
        "nekoui_rect_instances",
        "nekoui_rect_bind_group",
    );
    (buffer, bind_group)
}

impl RenderSystem {
    pub(super) fn ensure_rect_capacity(&mut self, count: usize) {
        if count <= self.rect_instance_capacity {
            return;
        }
        while self.rect_instance_capacity < count {
            self.rect_instance_capacity *= 2;
        }
        let (buffer, bind_group) = rebuild_rect_storage(
            &self.context.device,
            &self.rect_bind_group_layout,
            self.rect_instance_capacity,
        );
        self.rect_storage_buffer = buffer;
        self.rect_bind_group = bind_group;
        self.buffer_epoch = self.buffer_epoch.saturating_add(1);
    }

    pub(super) fn maybe_shrink_rect_capacity(&mut self, count: usize) {
        if self.rect_instance_capacity <= 64 {
            self.rect_low_usage_frames = 0;
            return;
        }

        if count.saturating_mul(4) > self.rect_instance_capacity {
            self.rect_low_usage_frames = 0;
            return;
        }

        self.rect_low_usage_frames += 1;
        if self.rect_low_usage_frames < SHRINK_IDLE_FRAME_THRESHOLD {
            return;
        }

        let target = count.max(1).saturating_mul(2).max(64).next_power_of_two();
        if target >= self.rect_instance_capacity {
            self.rect_low_usage_frames = 0;
            return;
        }

        self.rect_instance_capacity = target;
        let (buffer, bind_group) = rebuild_rect_storage(
            &self.context.device,
            &self.rect_bind_group_layout,
            self.rect_instance_capacity,
        );
        self.rect_storage_buffer = buffer;
        self.rect_bind_group = bind_group;
        self.rect_low_usage_frames = 0;
        self.buffer_epoch = self.buffer_epoch.saturating_add(1);
    }

    pub(super) fn ensure_mono_text_capacity(&mut self, count: usize) {
        if count <= self.mono_text_instance_capacity {
            return;
        }
        while self.mono_text_instance_capacity < count {
            self.mono_text_instance_capacity *= 2;
        }
        let (buffer, bind_group) = rebuild_storage::<TextInstance>(
            &self.context.device,
            &self.text_instance_bind_group_layout,
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
            self.buffer_epoch = self.buffer_epoch.saturating_add(1);
        }
    }

    pub(super) fn ensure_color_text_capacity(&mut self, count: usize) {
        if count <= self.color_text_instance_capacity {
            return;
        }
        while self.color_text_instance_capacity < count {
            self.color_text_instance_capacity *= 2;
        }
        let (buffer, bind_group) = rebuild_storage::<ColorTextInstance>(
            &self.context.device,
            &self.text_instance_bind_group_layout,
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
            self.buffer_epoch = self.buffer_epoch.saturating_add(1);
        }
    }
}
