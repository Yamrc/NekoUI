use crate::SharedString;
use crate::style::{
    Absolute, AlignItems, AlignSelf, BoxSizing, Color, Definite, Edges, FontStyle, FontWeight,
    IntoFontFamilies, LayoutSize, Length, Size, Style, TextAlign, TextOverflow, WhiteSpace,
};

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

    pub fn size(mut self, size: LayoutSize) -> Self {
        self.style.layout.size = size;
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.style.layout.size.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.style.layout.size.height = height.into();
        self
    }

    pub fn w(self, width: impl Into<Length>) -> Self {
        self.width(width)
    }

    pub fn h(self, height: impl Into<Length>) -> Self {
        self.height(height)
    }

    pub fn min_size(mut self, size: Size<Option<Definite>>) -> Self {
        self.style.layout.min_size = size;
        self
    }

    pub fn max_size(mut self, size: Size<Option<Definite>>) -> Self {
        self.style.layout.max_size = size;
        self
    }

    pub fn min_width(mut self, width: impl Into<Definite>) -> Self {
        self.style.layout.min_size.width = Some(width.into());
        self
    }

    pub fn min_w(self, width: impl Into<Definite>) -> Self {
        self.min_width(width)
    }

    pub fn min_height(mut self, height: impl Into<Definite>) -> Self {
        self.style.layout.min_size.height = Some(height.into());
        self
    }

    pub fn min_h(self, height: impl Into<Definite>) -> Self {
        self.min_height(height)
    }

    pub fn max_width(mut self, width: impl Into<Definite>) -> Self {
        self.style.layout.max_size.width = Some(width.into());
        self
    }

    pub fn max_w(self, width: impl Into<Definite>) -> Self {
        self.max_width(width)
    }

    pub fn max_height(mut self, height: impl Into<Definite>) -> Self {
        self.style.layout.max_size.height = Some(height.into());
        self
    }

    pub fn max_h(self, height: impl Into<Definite>) -> Self {
        self.max_height(height)
    }

    pub fn margin(mut self, margin: impl Into<Edges<Length>>) -> Self {
        self.style.layout.margin = margin.into();
        self
    }

    pub fn m(mut self, value: impl Into<Length>) -> Self {
        self.style.layout.margin = Edges::all(value.into());
        self
    }

    pub fn mx(mut self, value: impl Into<Length>) -> Self {
        let value = value.into();
        self.style.layout.margin.left = value;
        self.style.layout.margin.right = value;
        self
    }

    pub fn my(mut self, value: impl Into<Length>) -> Self {
        let value = value.into();
        self.style.layout.margin.top = value;
        self.style.layout.margin.bottom = value;
        self
    }

    pub fn align_self(mut self, align_self: AlignSelf) -> Self {
        self.style.layout.align_self = Some(align_self);
        self
    }

    pub fn self_center(self) -> Self {
        self.align_self(AlignItems::Center)
    }

    pub fn self_start(self) -> Self {
        self.align_self(AlignItems::Start)
    }

    pub fn self_end(self) -> Self {
        self.align_self(AlignItems::End)
    }

    pub fn self_stretch(self) -> Self {
        self.align_self(AlignItems::Stretch)
    }

    pub fn flex_grow(mut self, value: f32) -> Self {
        self.style.layout.flex_grow = value.max(0.0);
        self
    }

    pub fn flex_shrink(mut self, value: f32) -> Self {
        self.style.layout.flex_shrink = value.max(0.0);
        self
    }

    pub fn flex_basis(mut self, basis: impl Into<Length>) -> Self {
        self.style.layout.flex_basis = basis.into();
        self
    }

    pub fn box_sizing(mut self, box_sizing: BoxSizing) -> Self {
        self.style.layout.box_sizing = box_sizing;
        self
    }

    pub fn border_box(self) -> Self {
        self.box_sizing(BoxSizing::BorderBox)
    }

    pub fn content_box(self) -> Self {
        self.box_sizing(BoxSizing::ContentBox)
    }

    pub fn font_size(mut self, font_size: impl Into<Absolute>) -> Self {
        self.style.text.font_size = Some(font_size.into());
        self
    }

    pub fn line_height(mut self, line_height: impl Into<Definite>) -> Self {
        self.style.text.line_height = Some(line_height.into());
        self
    }

    pub fn font_family(mut self, families: impl IntoFontFamilies) -> Self {
        self.style.text.font_families = Some(families.into_font_families());
        self
    }

    pub fn text_color(self, color: Color) -> Self {
        self.color(color)
    }

    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.style.text.font_weight = Some(weight);
        self
    }

    pub fn bold(self) -> Self {
        self.font_weight(FontWeight::Bold)
    }

    pub fn font_style(mut self, style: FontStyle) -> Self {
        self.style.text.font_style = Some(style);
        self
    }

    pub fn italic(self) -> Self {
        self.font_style(FontStyle::Italic)
    }

    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.style.text.text_align = Some(align);
        self
    }

    pub fn text_center(self) -> Self {
        self.text_align(TextAlign::Center)
    }

    pub fn text_left(self) -> Self {
        self.text_align(TextAlign::Start)
    }

    pub fn text_right(self) -> Self {
        self.text_align(TextAlign::End)
    }

    pub fn white_space(mut self, white_space: WhiteSpace) -> Self {
        self.style.text.white_space = Some(white_space);
        self
    }

    pub fn whitespace_nowrap(self) -> Self {
        self.white_space(WhiteSpace::Nowrap)
    }

    pub fn whitespace_normal(self) -> Self {
        self.white_space(WhiteSpace::Normal)
    }

    pub fn text_overflow(mut self, overflow: TextOverflow) -> Self {
        self.style.text.text_overflow = Some(overflow);
        self
    }

    pub fn text_ellipsis(self) -> Self {
        self.text_overflow(TextOverflow::Ellipsis)
    }

    pub fn text_clip(self) -> Self {
        self.text_overflow(TextOverflow::Clip)
    }

    pub fn truncate(self) -> Self {
        self.whitespace_nowrap().text_ellipsis()
    }

    pub fn color(mut self, color: Color) -> Self {
        self.style.text.color = Some(color);
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
