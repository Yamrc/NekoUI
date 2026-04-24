mod attributes;
mod command;
mod handle;
mod model;
pub(crate) mod native;

pub(crate) use command::{WindowCommand, WindowCommandSender};
pub use handle::{WindowHandle, WindowId};
pub(crate) use model::WindowInfoSeed;
pub use model::{
    DisplayId, DisplayInfo, DisplaySelector, WindowAppearance, WindowBehavior, WindowGeometry,
    WindowGeometryPatch, WindowInfo, WindowOptions, WindowPlacement, WindowSize,
    WindowStartPosition,
};

pub(crate) use attributes::{
    active_displays, apply_geometry_patch, apply_post_create_state, current_display_id,
    current_frame_size, current_placement, current_position, update_hidden_titlebar_hit_test_state,
    window_attributes,
};
