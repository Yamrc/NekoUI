use crate::scene::CompiledScene;

pub(crate) struct RenderFramePackage<'a> {
    pub(crate) scene: &'a CompiledScene,
    pub(crate) metrics_generation: u64,
    pub(crate) scene_generation: u64,
    pub(crate) target_surface_generation: u64,
    pub(crate) scale_factor: f64,
}

impl<'a> RenderFramePackage<'a> {
    pub(crate) fn is_current(&self) -> bool {
        self.metrics_generation == self.scene_generation
    }

    pub(crate) fn matches_surface_generation(&self, current_surface_generation: u64) -> bool {
        self.target_surface_generation == current_surface_generation
    }
}
