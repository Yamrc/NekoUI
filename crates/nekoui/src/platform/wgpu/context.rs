use std::sync::Arc;

use wgpu::{
    Backends, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, Limits,
    MemoryHints, PowerPreference, Queue, RequestAdapterOptions, Trace,
};
use winit::window::Window as WinitWindow;

use crate::error::PlatformError;

pub(crate) struct WgpuContext {
    pub(crate) instance: Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: Device,
    pub(crate) queue: Queue,
    pub(crate) max_texture_size: u32,
}

impl WgpuContext {
    pub(crate) fn new(
        window: Arc<WinitWindow>,
    ) -> Result<(Self, wgpu::Surface<'static>), PlatformError> {
        let mut descriptor = InstanceDescriptor::new_without_display_handle();
        descriptor.backends = current_backends();
        let instance = Instance::new(descriptor);
        let surface = instance
            .create_surface(window)
            .map_err(|error| PlatformError::new(error.to_string()))?;

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::LowPower, // TODO: 改成可配置
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .map_err(|error| PlatformError::new(error.to_string()))?;

        let required_limits = Limits::downlevel_defaults()
            .using_resolution(adapter.limits())
            .using_alignment(adapter.limits());
        let supported_features = adapter.features();
        let required_features = supported_features & Features::PIPELINE_CACHE;
        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("nekoui_device"),
            required_features,
            required_limits,
            experimental_features: Default::default(),
            memory_hints: MemoryHints::MemoryUsage,
            trace: Trace::Off,
        }))
        .map_err(|error| PlatformError::new(error.to_string()))?;

        Ok((
            Self {
                instance,
                max_texture_size: adapter.limits().max_texture_dimension_2d.max(1),
                adapter,
                device,
                queue,
            },
            surface,
        ))
    }

    pub(crate) fn create_surface_for_window(
        &self,
        window: Arc<WinitWindow>,
    ) -> Result<wgpu::Surface<'static>, PlatformError> {
        self.instance
            .create_surface(window)
            .map_err(|error| PlatformError::new(error.to_string()))
    }
}

fn current_backends() -> Backends {
    // TODO: Windows额外增加D3D12
    #[cfg(target_os = "macos")]
    {
        Backends::METAL
    }
    #[cfg(not(target_os = "macos"))]
    {
        Backends::VULKAN
    }
}
