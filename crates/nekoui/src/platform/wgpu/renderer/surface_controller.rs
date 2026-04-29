use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::window::Window as WinitWindow;

use crate::error::PlatformError;
use crate::platform::wgpu::context::WgpuContext;
use crate::window::WindowSize;

const RESIZE_STABILITY_GRACE: Duration = Duration::from_millis(40);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceLifecycleState {
    Stable,
    ResizePending {
        requested: WindowSize,
        stable_after: Instant,
        session_peak_area: u32,
    },
    Occluded,
    Lost,
    Unavailable,
}

pub(crate) struct SurfaceController {
    pub(super) window: Arc<WinitWindow>,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) target_size: WindowSize,
    pub(super) surface_state: SurfaceLifecycleState,
    pub(super) config_generation: u64,
}

impl SurfaceController {
    pub(super) fn new(
        window: Arc<WinitWindow>,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        target_size: WindowSize,
    ) -> Self {
        Self {
            window,
            surface,
            config,
            target_size,
            surface_state: SurfaceLifecycleState::Stable,
            config_generation: 1,
        }
    }

    pub(super) fn mark_configured(&mut self) {
        self.surface_state = SurfaceLifecycleState::Stable;
        self.config_generation = self.config_generation.saturating_add(1);
    }

    pub(super) fn note_resize(&mut self, physical_size: WindowSize) {
        self.target_size = physical_size;
        if physical_size.width == 0 || physical_size.height == 0 {
            self.surface_state = SurfaceLifecycleState::Unavailable;
            return;
        }

        let peak_area = match self.surface_state {
            SurfaceLifecycleState::ResizePending {
                session_peak_area, ..
            } => session_peak_area.max(physical_size.width.saturating_mul(physical_size.height)),
            _ => physical_size.width.saturating_mul(physical_size.height),
        };
        self.surface_state = SurfaceLifecycleState::ResizePending {
            requested: physical_size,
            stable_after: Instant::now() + RESIZE_STABILITY_GRACE,
            session_peak_area: peak_area,
        };
    }

    pub(super) fn note_occlusion(&mut self, occluded: bool, max_texture_size: u32) {
        if occluded {
            self.surface_state = SurfaceLifecycleState::Occluded;
            return;
        }

        if self.target_size.width == 0 || self.target_size.height == 0 {
            self.surface_state = SurfaceLifecycleState::Unavailable;
            return;
        }

        let target_size = surface_extent_for(self.target_size, max_texture_size);
        self.surface_state =
            if self.config.width == target_size.width && self.config.height == target_size.height {
                SurfaceLifecycleState::Stable
            } else {
                SurfaceLifecycleState::ResizePending {
                    requested: self.target_size,
                    stable_after: Instant::now() + RESIZE_STABILITY_GRACE,
                    session_peak_area: self
                        .target_size
                        .width
                        .saturating_mul(self.target_size.height),
                }
            };
    }

    pub(super) fn recreate(&mut self, context: &WgpuContext) -> Result<(), PlatformError> {
        self.surface = context.create_surface_for_window(self.window.clone())?;
        let target_size = surface_extent_for(self.target_size, context.max_texture_size);
        let mut config = self
            .surface
            .get_default_config(&context.adapter, target_size.width, target_size.height)
            .ok_or_else(|| PlatformError::new("surface has no default configuration"))?;
        config.desired_maximum_frame_latency = 1;
        self.config = config;
        self.note_resize(self.target_size);
        Ok(())
    }

    pub(super) fn configure_if_needed(
        &mut self,
        device: &wgpu::Device,
        max_texture_size: u32,
    ) -> bool {
        let target_size = surface_extent_for(self.target_size, max_texture_size);
        let configured_matches =
            self.config.width == target_size.width && self.config.height == target_size.height;

        let needs_configure = match self.surface_state {
            SurfaceLifecycleState::ResizePending { .. } | SurfaceLifecycleState::Lost => true,
            SurfaceLifecycleState::Occluded | SurfaceLifecycleState::Unavailable => {
                !configured_matches
            }
            SurfaceLifecycleState::Stable => !configured_matches,
        };

        if !needs_configure {
            self.surface_state = SurfaceLifecycleState::Stable;
            return false;
        }

        self.config.width = target_size.width;
        self.config.height = target_size.height;
        self.surface.configure(device, &self.config);
        self.mark_configured();
        true
    }
}

pub(super) fn surface_extent_for(physical_size: WindowSize, max_texture_size: u32) -> WindowSize {
    WindowSize::new(
        physical_size.width.max(1).min(max_texture_size),
        physical_size.height.max(1).min(max_texture_size),
    )
}
