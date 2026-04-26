use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::monitor::MonitorHandle;

use crate::SharedString;
use crate::style::{Bounds, Point, Px, Size, bounds, point, px, size};

use super::handle::WindowId;

const DEFAULT_WINDOW_TITLE: &str = "NekoUI";
const DEFAULT_WINDOW_WIDTH: f32 = 960.0;
const DEFAULT_WINDOW_HEIGHT: f32 = 640.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl WindowSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowPlacement {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAppearance {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowStartPosition {
    Default,
    Absolute(Point<Px>),
    Centered,
    CenteredOn(DisplaySelector),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaySelector {
    Primary,
    Active,
    ById(DisplayId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowGeometry {
    pub start_position: WindowStartPosition,
    pub size: Size<Px>,
    pub min_size: Option<Size<Px>>,
    pub max_size: Option<Size<Px>>,
    pub placement: WindowPlacement,
}

impl WindowGeometry {
    pub fn new(size: Size<Px>) -> Self {
        Self {
            start_position: WindowStartPosition::Default,
            size,
            min_size: None,
            max_size: None,
            placement: WindowPlacement::Windowed,
        }
    }

    pub fn position(mut self, position: WindowStartPosition) -> Self {
        self.start_position = position;
        self
    }

    pub fn min_size(mut self, size: Size<Px>) -> Self {
        self.min_size = Some(size);
        self
    }

    pub fn max_size(mut self, size: Size<Px>) -> Self {
        self.max_size = Some(size);
        self
    }

    pub fn placement(mut self, placement: WindowPlacement) -> Self {
        self.placement = placement;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowBehavior {
    pub start_visible: bool,
    pub start_focused: bool,
    pub resizable: bool,
}

impl WindowBehavior {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.start_visible = visible;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.start_focused = focused;
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
}

impl Default for WindowBehavior {
    fn default() -> Self {
        Self {
            start_visible: true,
            start_focused: true,
            resizable: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowOptions {
    pub title: SharedString,
    pub geometry: WindowGeometry,
    pub behavior: WindowBehavior,
    pub show_titlebar: bool,
    pub appearance: Option<WindowAppearance>,
}

impl WindowOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn geometry(mut self, geometry: WindowGeometry) -> Self {
        self.geometry = geometry;
        self
    }

    pub fn behavior(mut self, behavior: WindowBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    pub fn show_titlebar(mut self, show: bool) -> Self {
        self.show_titlebar = show;
        self
    }

    pub fn appearance(mut self, appearance: WindowAppearance) -> Self {
        self.appearance = Some(appearance);
        self
    }
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: SharedString::from(DEFAULT_WINDOW_TITLE),
            geometry: WindowGeometry::new(size(
                px(DEFAULT_WINDOW_WIDTH),
                px(DEFAULT_WINDOW_HEIGHT),
            )),
            behavior: WindowBehavior::default(),
            show_titlebar: true,
            appearance: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowGeometryPatch {
    pub position: Option<WindowStartPosition>,
    pub size: Option<Size<Px>>,
    pub min_size: Option<Option<Size<Px>>>,
    pub max_size: Option<Option<Size<Px>>>,
    pub placement: Option<WindowPlacement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    pub id: DisplayId,
    pub name: Option<SharedString>,
    pub bounds: Bounds<Px>,
    pub work_area: Bounds<Px>,
    pub scale_factor: f64,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WindowInfoSeed {
    pub content_size: WindowSize,
    pub frame_size: Option<WindowSize>,
    pub physical_size: WindowSize,
    pub scale_factor: f64,
    pub position: Option<Point<Px>>,
    pub current_display: Option<DisplayId>,
}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    id: WindowId,
    title: SharedString,
    placement: WindowPlacement,
    position: Option<Point<Px>>,
    content_size: WindowSize,
    frame_size: Option<WindowSize>,
    physical_size: WindowSize,
    scale_factor: f64,
    focused: bool,
    visible: bool,
    resizable: bool,
    show_titlebar: bool,
    appearance: Option<WindowAppearance>,
    current_display: Option<DisplayId>,
}

impl WindowInfo {
    pub(crate) fn from_options(
        id: WindowId,
        options: &WindowOptions,
        seed: WindowInfoSeed,
    ) -> Self {
        Self {
            id,
            title: options.title.clone(),
            placement: options.geometry.placement,
            position: seed.position,
            content_size: seed.content_size,
            frame_size: seed.frame_size,
            physical_size: seed.physical_size,
            scale_factor: sanitize_scale_factor(seed.scale_factor),
            focused: options.behavior.start_focused,
            visible: options.behavior.start_visible,
            resizable: options.behavior.resizable,
            show_titlebar: options.show_titlebar,
            appearance: options.appearance,
            current_display: seed.current_display,
        }
    }

    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn placement(&self) -> WindowPlacement {
        self.placement
    }

    pub fn position(&self) -> Option<Point<Px>> {
        self.position
    }

    pub fn content_size(&self) -> WindowSize {
        self.content_size
    }

    pub fn frame_size(&self) -> Option<WindowSize> {
        self.frame_size
    }

    pub fn physical_size(&self) -> WindowSize {
        self.physical_size
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn resizable(&self) -> bool {
        self.resizable
    }

    pub fn show_titlebar(&self) -> bool {
        self.show_titlebar
    }

    pub fn appearance(&self) -> Option<WindowAppearance> {
        self.appearance
    }

    pub fn current_display(&self) -> Option<DisplayId> {
        self.current_display
    }

    pub(crate) fn set_content_metrics(
        &mut self,
        content_size: WindowSize,
        frame_size: Option<WindowSize>,
        physical_size: WindowSize,
        scale_factor: f64,
    ) {
        self.content_size = content_size;
        self.frame_size = frame_size;
        self.physical_size = physical_size;
        self.scale_factor = sanitize_scale_factor(scale_factor);
    }

    pub(crate) fn set_position(&mut self, position: Option<Point<Px>>) {
        self.position = position;
    }

    pub(crate) fn set_placement(&mut self, placement: WindowPlacement) {
        self.placement = placement;
    }

    pub(crate) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub(crate) fn set_resizable(&mut self, resizable: bool) {
        self.resizable = resizable;
    }

    pub(crate) fn set_title(&mut self, title: SharedString) {
        self.title = title;
    }

    pub(crate) fn set_current_display(&mut self, current_display: Option<DisplayId>) {
        self.current_display = current_display;
    }
}

pub(crate) fn display_info_from_monitor(monitor: &MonitorHandle, is_primary: bool) -> DisplayInfo {
    let scale_factor = sanitize_scale_factor(monitor.scale_factor());
    let scale = scale_factor as f32;
    let size_value = monitor.size();
    let position_value = monitor.position();
    let id = display_id_from_monitor(monitor);
    let bounds = bounds(
        point(
            px(position_value.x as f32 / scale),
            px(position_value.y as f32 / scale),
        ),
        size(
            px(size_value.width as f32 / scale),
            px(size_value.height as f32 / scale),
        ),
    );

    DisplayInfo {
        id,
        name: monitor.name().map(SharedString::from),
        bounds,
        work_area: bounds,
        scale_factor,
        is_primary,
    }
}

pub(crate) fn display_id_from_monitor(monitor: &MonitorHandle) -> DisplayId {
    let mut hasher = DefaultHasher::new();
    monitor.name().hash(&mut hasher);
    monitor.position().x.hash(&mut hasher);
    monitor.position().y.hash(&mut hasher);
    monitor.size().width.hash(&mut hasher);
    monitor.size().height.hash(&mut hasher);
    monitor.scale_factor().to_bits().hash(&mut hasher);
    DisplayId(hasher.finish())
}

pub(crate) fn physical_size_to_window_size(size: PhysicalSize<u32>) -> WindowSize {
    WindowSize::new(size.width, size.height)
}

pub(crate) fn logical_position_from_physical(
    position: PhysicalPosition<i32>,
    scale_factor: f64,
) -> Point<Px> {
    let scale_factor = sanitize_scale_factor(scale_factor) as f32;
    Point::new(
        px(position.x as f32 / scale_factor),
        px(position.y as f32 / scale_factor),
    )
}

fn sanitize_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use crate::style::{point, px, size};

    use super::{
        DisplaySelector, WindowBehavior, WindowGeometry, WindowOptions, WindowPlacement,
        WindowStartPosition,
    };

    #[test]
    fn window_options_default_to_windowed_visible_window() {
        let options = WindowOptions::default();

        assert_eq!(options.title.as_ref(), "NekoUI");
        assert_eq!(options.geometry.placement, WindowPlacement::Windowed);
        assert_eq!(options.geometry.size, size(px(960.0), px(640.0)));
        assert!(options.behavior.start_focused);
        assert!(options.behavior.start_visible);
        assert!(options.behavior.resizable);
        assert!(options.show_titlebar);
    }

    #[test]
    fn builder_methods_override_window_geometry() {
        let options = WindowOptions::new()
            .title("Ame")
            .show_titlebar(false)
            .geometry(
                WindowGeometry::new(size(px(800.0), px(600.0)))
                    .position(WindowStartPosition::CenteredOn(DisplaySelector::Primary))
                    .placement(WindowPlacement::Maximized),
            )
            .behavior(WindowBehavior::new().visible(false).focused(false));

        assert_eq!(options.title.as_ref(), "Ame");
        assert!(!options.show_titlebar);
        assert_eq!(options.geometry.placement, WindowPlacement::Maximized);
        assert_eq!(
            options.geometry.start_position,
            WindowStartPosition::CenteredOn(DisplaySelector::Primary)
        );
        assert_eq!(options.geometry.size, size(px(800.0), px(600.0)));
        assert!(!options.behavior.start_visible);
        assert!(!options.behavior.start_focused);
    }

    #[test]
    fn absolute_position_is_preserved() {
        let geometry = WindowGeometry::new(size(px(320.0), px(240.0)))
            .position(WindowStartPosition::Absolute(point(px(12.0), px(18.0))));

        assert_eq!(
            geometry.start_position,
            WindowStartPosition::Absolute(point(px(12.0), px(18.0)))
        );
    }
}
