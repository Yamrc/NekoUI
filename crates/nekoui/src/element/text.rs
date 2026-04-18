use crate::SharedString;
use crate::style::{Color, Style};

use super::{AnyElement, IntoElement};

#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub(crate) key: Option<u64>,
    pub(crate) style: Style,
    pub(crate) content: SharedString,
}

pub fn text(content: impl Into<SharedString>) -> Text {
    Text {
        key: None,
        style: Style::default(),
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
}

impl IntoElement for Text {
    fn into_any_element(self) -> AnyElement {
        AnyElement::text(self)
    }
}
