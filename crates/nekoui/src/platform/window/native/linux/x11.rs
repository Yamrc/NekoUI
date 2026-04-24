use winit::window::Window as WinitWindow;

use super::{LinuxDecorationsMode, LinuxWindowRoute};

pub(crate) fn apply_post_create(window: &WinitWindow, route: LinuxWindowRoute) {
    log::debug!(
        "linux/x11 apply_post_create decorations={:?}",
        route.decorations
    );
    window.set_decorations(matches!(route.decorations, LinuxDecorationsMode::Server));
}
