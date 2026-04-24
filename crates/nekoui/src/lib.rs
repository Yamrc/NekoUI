mod app;
mod element;
pub mod error;
pub mod geometry;
mod platform;
mod scene;
pub mod style;
pub mod text_system;
pub mod window;

pub type SharedString = std::sync::Arc<str>;

pub use app::{
    AppContext, Application, BackgroundExecutor, Context, Entity, EventEmitter, LastWindowBehavior,
    Render, Subscription, Task, TaskResult, UiExecutor, View, WeakEntity,
};
pub use element::{
    AnyElement, Div, Fragment, IntoElement, IntoElements, ParentElement, Text, div, text,
};
pub use error::{Error, PlatformError, RuntimeError};
pub use geometry::{Bounds, Point, Px, Size, bounds, point, px, size};
pub use style::{
    AlignItems, BackgroundFill, Color, CornerRadii, Direction, EdgeInsets, EdgeWidths,
    JustifyContent, LayoutSize, LayoutStyle, Length, LinearGradient, PaintStyle, Style, TextStyle,
    gradient,
};
pub use text_system::{
    SharedTextLayout, TextCacheStats, TextLayout, TextMeasureKey, TextRun, TextSystem,
};
pub use window::{
    DisplayId, DisplayInfo, DisplaySelector, WindowAppearance, WindowBehavior, WindowGeometry,
    WindowGeometryPatch, WindowHandle, WindowId, WindowInfo, WindowOptions, WindowPlacement,
    WindowSize, WindowStartPosition,
};
