use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Theme, Window as WinitWindow, WindowAttributes};

use crate::element::WindowFrameArea;
use crate::scene::LayoutBox;
use crate::style::{Point, Px, Size};

use super::model::{
    DisplayId, DisplayInfo, DisplaySelector, WindowAppearance, WindowGeometryPatch, WindowOptions,
    WindowPlacement, WindowSize, WindowStartPosition, display_id_from_monitor,
    display_info_from_monitor, logical_position_from_physical, physical_size_to_window_size,
};

pub(crate) fn window_attributes(options: &WindowOptions) -> WindowAttributes {
    let attributes = WinitWindow::default_attributes()
        .with_title(options.title.to_string())
        .with_inner_size(to_logical_size(options.geometry.size))
        .with_resizable(options.behavior.resizable)
        .with_visible(options.behavior.start_visible)
        .with_active(options.behavior.start_focused)
        .with_theme(options.appearance.map(Theme::from));

    let attributes =
        if let WindowStartPosition::Absolute(position) = options.geometry.start_position {
            attributes.with_position(to_logical_position(position))
        } else {
            attributes
        };
    let attributes = if let Some(min_size) = options.geometry.min_size {
        attributes.with_min_inner_size(to_logical_size(min_size))
    } else {
        attributes
    };
    let attributes = if let Some(max_size) = options.geometry.max_size {
        attributes.with_max_inner_size(to_logical_size(max_size))
    } else {
        attributes
    };

    decorate_attributes_for_platform(attributes, options)
}

pub(crate) fn active_displays(event_loop: &ActiveEventLoop) -> Vec<DisplayInfo> {
    let primary_id = event_loop
        .primary_monitor()
        .as_ref()
        .map(display_id_from_monitor);
    event_loop
        .available_monitors()
        .map(|monitor| {
            let id = display_id_from_monitor(&monitor);
            display_info_from_monitor(&monitor, Some(id) == primary_id)
        })
        .collect()
}

pub(crate) fn apply_post_create_state(
    event_loop: &ActiveEventLoop,
    window: &WinitWindow,
    options: &WindowOptions,
    displays: &[DisplayInfo],
    active_display: Option<DisplayId>,
) {
    apply_position_request(window, options, displays, active_display);
    match options.geometry.placement {
        WindowPlacement::Windowed => {}
        WindowPlacement::Maximized => window.set_maximized(true),
        WindowPlacement::Fullscreen => {
            let monitor =
                resolve_monitor(window, displays, active_display, DisplaySelector::Active);
            window.set_fullscreen(Some(Fullscreen::Borderless(monitor)));
        }
    }
    platform_apply_post_create(event_loop, window, options);
}

pub(crate) fn apply_geometry_patch(
    window: &WinitWindow,
    patch: &WindowGeometryPatch,
    displays: &[DisplayInfo],
    active_display: Option<DisplayId>,
) {
    if let Some(position) = patch.position {
        match position {
            WindowStartPosition::Default => {}
            WindowStartPosition::Absolute(position) => {
                window.set_outer_position(to_logical_position(position));
            }
            WindowStartPosition::Centered => {
                if let Some(position) =
                    center_in_display(window, displays, active_display, DisplaySelector::Active)
                {
                    window.set_outer_position(to_logical_position(position));
                }
            }
            WindowStartPosition::CenteredOn(selector) => {
                if let Some(position) =
                    center_in_display(window, displays, active_display, selector)
                {
                    window.set_outer_position(to_logical_position(position));
                }
            }
        }
    }

    if let Some(size) = patch.size {
        let _ = window.request_inner_size(to_logical_size(size));
    }
    if let Some(min_size) = patch.min_size {
        window.set_min_inner_size(min_size.map(to_logical_size));
    }
    if let Some(max_size) = patch.max_size {
        window.set_max_inner_size(max_size.map(to_logical_size));
    }
    if let Some(placement) = patch.placement {
        match placement {
            WindowPlacement::Windowed => {
                window.set_fullscreen(None);
                window.set_maximized(false);
            }
            WindowPlacement::Maximized => {
                window.set_fullscreen(None);
                window.set_maximized(true);
            }
            WindowPlacement::Fullscreen => {
                let monitor =
                    resolve_monitor(window, displays, active_display, DisplaySelector::Active);
                window.set_fullscreen(Some(Fullscreen::Borderless(monitor)));
            }
        }
    }
}

pub(crate) fn current_placement(window: &WinitWindow) -> WindowPlacement {
    if window.fullscreen().is_some() {
        WindowPlacement::Fullscreen
    } else if window.is_maximized() {
        WindowPlacement::Maximized
    } else {
        WindowPlacement::Windowed
    }
}

pub(crate) fn current_position(window: &WinitWindow, scale_factor: f64) -> Option<Point<Px>> {
    window
        .outer_position()
        .ok()
        .map(|position| logical_position_from_physical(position, scale_factor))
}

pub(crate) fn current_frame_size(window: &WinitWindow) -> Option<WindowSize> {
    Some(physical_size_to_window_size(window.outer_size()))
}

pub(crate) fn current_display_id(window: &WinitWindow) -> Option<DisplayId> {
    window
        .current_monitor()
        .as_ref()
        .map(display_id_from_monitor)
}

#[cfg(target_os = "windows")]
pub(crate) fn update_hidden_titlebar_hit_test_state(
    window: &WinitWindow,
    scale_factor: f64,
    areas: &[(WindowFrameArea, LayoutBox)],
) {
    super::native::windows::update_hidden_titlebar_hit_test_state(window, scale_factor, areas);
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn update_hidden_titlebar_hit_test_state(
    _window: &WinitWindow,
    _scale_factor: f64,
    _areas: &[(WindowFrameArea, LayoutBox)],
) {
}

fn decorate_attributes_for_platform(
    attributes: WindowAttributes,
    options: &WindowOptions,
) -> WindowAttributes {
    #[cfg(target_os = "windows")]
    {
        return super::native::windows::decorate_attributes(attributes, options);
    }
    #[cfg(target_os = "macos")]
    {
        return super::native::macos::decorate_attributes(attributes, options);
    }
    #[cfg(target_os = "linux")]
    {
        return super::native::linux::decorate_attributes(attributes, options);
    }
    #[allow(unreachable_code)]
    attributes
}

fn platform_apply_post_create(
    event_loop: &ActiveEventLoop,
    window: &WinitWindow,
    options: &WindowOptions,
) {
    #[cfg(target_os = "windows")]
    {
        let _ = event_loop;
        super::native::windows::apply_post_create(window, options);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = event_loop;
        super::native::macos::apply_post_create(window, options);
    }
    #[cfg(target_os = "linux")]
    {
        let backend = super::native::linux::backend_kind(event_loop);
        super::native::linux::apply_post_create(backend, window, options);
    }
}

fn apply_position_request(
    window: &WinitWindow,
    options: &WindowOptions,
    displays: &[DisplayInfo],
    active_display: Option<DisplayId>,
) {
    let target_position = match options.geometry.start_position {
        WindowStartPosition::Default => None,
        WindowStartPosition::Absolute(position) => Some(position),
        WindowStartPosition::Centered => {
            center_in_display(window, displays, active_display, DisplaySelector::Active)
        }
        WindowStartPosition::CenteredOn(selector) => {
            center_in_display(window, displays, active_display, selector)
        }
    };

    if let Some(position) = target_position {
        window.set_outer_position(to_logical_position(position));
    }
}

fn center_in_display(
    window: &WinitWindow,
    displays: &[DisplayInfo],
    active_display: Option<DisplayId>,
    selector: DisplaySelector,
) -> Option<Point<Px>> {
    let display = resolve_display_info(displays, active_display, selector)?;
    let outer = current_frame_size(window)?;
    let width = outer.width as f32 / window.scale_factor() as f32;
    let height = outer.height as f32 / window.scale_factor() as f32;
    let x = display.bounds.origin.x.get() + (display.bounds.size.width.get() - width) * 0.5;
    let y = display.bounds.origin.y.get() + (display.bounds.size.height.get() - height) * 0.5;
    Some(Point::new(Px(x), Px(y)))
}

fn resolve_display_info(
    displays: &[DisplayInfo],
    active_display: Option<DisplayId>,
    selector: DisplaySelector,
) -> Option<DisplayInfo> {
    match selector {
        DisplaySelector::Primary => displays.iter().find(|display| display.is_primary).cloned(),
        DisplaySelector::Active => active_display
            .and_then(|active| {
                displays
                    .iter()
                    .find(|display| display.id == active)
                    .cloned()
            })
            .or_else(|| displays.iter().find(|display| display.is_primary).cloned()),
        DisplaySelector::ById(id) => displays.iter().find(|display| display.id == id).cloned(),
    }
}

fn resolve_monitor(
    window: &WinitWindow,
    displays: &[DisplayInfo],
    active_display: Option<DisplayId>,
    selector: DisplaySelector,
) -> Option<MonitorHandle> {
    let target = resolve_display_info(displays, active_display, selector)?;
    window
        .available_monitors()
        .find(|monitor| display_id_from_monitor(monitor) == target.id)
}

fn to_logical_size(size_value: Size<Px>) -> LogicalSize<f64> {
    LogicalSize::new(
        f64::from(size_value.width.get()),
        f64::from(size_value.height.get()),
    )
}

fn to_logical_position(position: Point<Px>) -> LogicalPosition<f64> {
    LogicalPosition::new(f64::from(position.x.get()), f64::from(position.y.get()))
}

impl From<WindowAppearance> for Theme {
    fn from(value: WindowAppearance) -> Self {
        match value {
            WindowAppearance::Light => Theme::Light,
            WindowAppearance::Dark => Theme::Dark,
        }
    }
}
