use crate::SharedString;
use crate::style::{Color, Style};

use super::{AnyElement, IntoElement, WindowFrameArea};

#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub(crate) key: Option<u64>,
    pub(crate) style: Style,
    pub(crate) window_frame_area: Option<WindowFrameArea>,
    pub(crate) content: SharedString,
}

pub fn text(content: impl Into<SharedString>) -> Text {
    Text {
        key: None,
        style: Style::default(),
        window_frame_area: None,
        content: content.into(),
    }
}

impl Text {
    pub fn key(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.style.text.font_size = font_size.max(1.0);
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.style.text.line_height = Some(line_height.max(1.0));
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.style.text.font_family = Some(family.into());
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.style.text.color = color;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.paint.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn window_drag_area(mut self) -> Self {
        self.window_frame_area = Some(WindowFrameArea::Drag);
        self
    }

    pub fn window_close_button(mut self) -> Self {
        self.window_frame_area = Some(WindowFrameArea::Close);
        self
    }

    pub fn window_maximize_button(mut self) -> Self {
        self.window_frame_area = Some(WindowFrameArea::Maximize);
        self
    }

    pub fn window_minimize_button(mut self) -> Self {
        self.window_frame_area = Some(WindowFrameArea::Minimize);
        self
    }
}

impl IntoElement for Text {
    fn into_any_element(self) -> AnyElement {
        AnyElement::text(self)
    }
}
