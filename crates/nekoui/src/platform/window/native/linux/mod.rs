pub(crate) mod wayland;
pub(crate) mod x11;

use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, ResizeDirection, Window as WinitWindow, WindowAttributes};

use crate::platform::window::{WindowInfo, WindowOptions};

const CLIENT_DECORATION_GRIP_THICKNESS_LOGICAL_PX: f64 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinuxBackendKind {
    X11,
    Wayland,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinuxDecorationsMode {
    Server,
    Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LinuxWindowRoute {
    pub backend: LinuxBackendKind,
    pub decorations: LinuxDecorationsMode,
}

pub(crate) fn backend_kind(event_loop: &ActiveEventLoop) -> LinuxBackendKind {
    use winit::platform::wayland::ActiveEventLoopExtWayland;

    if event_loop.is_wayland() {
        LinuxBackendKind::Wayland
    } else {
        LinuxBackendKind::X11
    }
}

pub(crate) fn route_for(backend: LinuxBackendKind, options: &WindowOptions) -> LinuxWindowRoute {
    let decorations = if options.show_titlebar {
        LinuxDecorationsMode::Server
    } else {
        LinuxDecorationsMode::Client
    };

    LinuxWindowRoute {
        backend,
        decorations,
    }
}

pub(crate) fn decorate_attributes(
    attributes: WindowAttributes,
    options: &WindowOptions,
) -> WindowAttributes {
    // Linux backends only expose a single decorations switch at create-time.
    // `false` follows the client-decoration direction, which is the closest path to NekoWG.
    attributes.with_decorations(options.show_titlebar)
}

pub(crate) fn apply_post_create(
    backend: LinuxBackendKind,
    window: &WinitWindow,
    options: &WindowOptions,
) {
    let route = route_for(backend, options);
    match route.backend {
        LinuxBackendKind::X11 => x11::apply_post_create(window, route),
        LinuxBackendKind::Wayland => wayland::apply_post_create(window, route),
    }
}

pub(crate) fn update_client_decorations_cursor(
    route: LinuxWindowRoute,
    native_window: &WinitWindow,
    window: &WindowInfo,
    cursor_position: PhysicalPosition<f64>,
) {
    if let Some(direction) = client_resize_direction(route, native_window, window, cursor_position)
    {
        native_window.set_cursor(CursorIcon::from(direction));
    } else if matches!(route.decorations, LinuxDecorationsMode::Client) {
        native_window.set_cursor(CursorIcon::Default);
    }
}

pub(crate) fn clear_client_decorations_cursor(
    route: LinuxWindowRoute,
    native_window: &WinitWindow,
) {
    if matches!(route.decorations, LinuxDecorationsMode::Client) {
        native_window.set_cursor(CursorIcon::Default);
    }
}

pub(crate) fn begin_client_decorations_resize(
    route: LinuxWindowRoute,
    native_window: &WinitWindow,
    window: &WindowInfo,
    cursor_position: Option<PhysicalPosition<f64>>,
) {
    let Some(cursor_position) = cursor_position else {
        return;
    };
    let Some(direction) = client_resize_direction(route, native_window, window, cursor_position)
    else {
        return;
    };

    if let Err(error) = native_window.drag_resize_window(direction) {
        log::debug!("linux client-decoration drag_resize_window unavailable: {error}");
    }
}

fn client_resize_direction(
    route: LinuxWindowRoute,
    native_window: &WinitWindow,
    window: &WindowInfo,
    cursor_position: PhysicalPosition<f64>,
) -> Option<ResizeDirection> {
    if !matches!(route.decorations, LinuxDecorationsMode::Client) || !window.resizable() {
        return None;
    }

    if matches!(
        window.placement(),
        crate::window::WindowPlacement::Maximized | crate::window::WindowPlacement::Fullscreen
    ) {
        return None;
    }

    let grip = (window.scale_factor() * CLIENT_DECORATION_GRIP_THICKNESS_LOGICAL_PX).max(4.0);
    let inner_size = native_window.inner_size();
    let width = inner_size.width as f64;
    let height = inner_size.height as f64;
    let x = cursor_position.x;
    let y = cursor_position.y;

    if x < 0.0 || y < 0.0 || x > width || y > height {
        return None;
    }

    let left = x <= grip;
    let right = x >= width - grip;
    let top = y <= grip;
    let bottom = y >= height - grip;

    match (left, right, top, bottom) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use winit::dpi::{PhysicalPosition, PhysicalSize};
    use winit::window::ResizeDirection;

    use super::{LinuxBackendKind, LinuxDecorationsMode, route_for};
    use crate::window::WindowOptions;

    fn detect_resize_direction(
        size: PhysicalSize<u32>,
        cursor: PhysicalPosition<f64>,
        grip: f64,
    ) -> Option<ResizeDirection> {
        let width = size.width as f64;
        let height = size.height as f64;
        let x = cursor.x;
        let y = cursor.y;

        if x < 0.0 || y < 0.0 || x > width || y > height {
            return None;
        }

        let left = x <= grip;
        let right = x >= width - grip;
        let top = y <= grip;
        let bottom = y >= height - grip;

        match (left, right, top, bottom) {
            (true, _, true, _) => Some(ResizeDirection::NorthWest),
            (_, true, true, _) => Some(ResizeDirection::NorthEast),
            (true, _, _, true) => Some(ResizeDirection::SouthWest),
            (_, true, _, true) => Some(ResizeDirection::SouthEast),
            (true, _, _, _) => Some(ResizeDirection::West),
            (_, true, _, _) => Some(ResizeDirection::East),
            (_, _, true, _) => Some(ResizeDirection::North),
            (_, _, _, true) => Some(ResizeDirection::South),
            _ => None,
        }
    }

    #[test]
    fn titlebarless_windows_follow_client_decoration_route() {
        let route = route_for(
            LinuxBackendKind::Wayland,
            &WindowOptions::default().show_titlebar(false),
        );

        assert_eq!(route.decorations, LinuxDecorationsMode::Client);
    }

    #[test]
    fn titled_windows_follow_server_decoration_route() {
        let route = route_for(LinuxBackendKind::X11, &WindowOptions::default());

        assert_eq!(route.decorations, LinuxDecorationsMode::Server);
    }

    #[test]
    fn client_resize_detects_top_edge_and_corners() {
        let size = PhysicalSize::new(800, 600);
        let grip = 8.0;

        assert_eq!(
            detect_resize_direction(size, PhysicalPosition::new(2.0, 2.0), grip),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(
            detect_resize_direction(size, PhysicalPosition::new(798.0, 2.0), grip),
            Some(ResizeDirection::NorthEast)
        );
        assert_eq!(
            detect_resize_direction(size, PhysicalPosition::new(400.0, 2.0), grip),
            Some(ResizeDirection::North)
        );
    }
}
