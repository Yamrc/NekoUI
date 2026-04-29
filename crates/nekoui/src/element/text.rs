use crate::SharedString;
use crate::style::{
    Absolute, AlignItems, AlignSelf, BoxSizing, Color, Definite, Edges, FontStyle, FontWeight,
    IntoFontFamilies, LayoutSize, Length, Size, Style, TextAlign, TextOverflow, WhiteSpace,
};

use super::builder_macros::{
    impl_shared_flex_item_builders, impl_shared_key_size_margin_builders,
    impl_shared_text_style_builders, impl_shared_window_chrome_builders,
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
    impl_shared_key_size_margin_builders!();
    impl_shared_flex_item_builders!();
    impl_shared_text_style_builders!();

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

    impl_shared_window_chrome_builders!();
}

impl IntoElement for Text {
    fn into_any_element(self) -> AnyElement {
        AnyElement::text(self)
    }
}
