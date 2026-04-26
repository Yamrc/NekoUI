use crate::style::{
    Absolute, AlignItems, AlignSelf, Background, Border, BoxSizing, Color, CornerRadii, Definite,
    Direction, Display, EdgeWidths, Edges, FlexDirection, FlexWrap, FontStyle, FontWeight, Gap,
    IntoFontFamilies, JustifyContent, LayoutSize, Length, Overflow, Size, Style, TextAlign,
    WhiteSpace,
};

use super::{AnyElement, Fragment, IntoElement, IntoElements, ParentElement, WindowFrameArea};

#[derive(Debug, Clone, PartialEq)]
pub struct Div {
    pub(crate) key: Option<u64>,
    pub(crate) style: Style,
    pub(crate) window_frame_area: Option<WindowFrameArea>,
    pub(crate) children: Fragment,
}

pub fn div() -> Div {
    Div {
        key: None,
        style: Style::default(),
        window_frame_area: None,
        children: Fragment::new(),
    }
}

impl Default for Div {
    fn default() -> Self {
        div()
    }
}

impl Div {
    pub fn key(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }

    pub fn size(mut self, size: LayoutSize) -> Self {
        self.style.layout.size = size;
        self
    }

    pub fn min_size(mut self, size: Size<Option<Definite>>) -> Self {
        self.style.layout.min_size = size;
        self
    }

    pub fn max_size(mut self, size: Size<Option<Definite>>) -> Self {
        self.style.layout.max_size = size;
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

    pub fn w(self, width: impl Into<Length>) -> Self {
        self.width(width)
    }

    pub fn h(self, height: impl Into<Length>) -> Self {
        self.height(height)
    }

    pub fn padding(mut self, padding: impl Into<Edges<Definite>>) -> Self {
        self.style.layout.padding = padding.into();
        self
    }

    pub fn margin(mut self, margin: impl Into<Edges<Length>>) -> Self {
        self.style.layout.margin = margin.into();
        self
    }

    pub fn p(mut self, value: impl Into<Definite>) -> Self {
        self.style.layout.padding = Edges::all(value.into());
        self
    }

    pub fn px(mut self, value: impl Into<Definite>) -> Self {
        let value = value.into();
        self.style.layout.padding.left = value;
        self.style.layout.padding.right = value;
        self
    }

    pub fn py(mut self, value: impl Into<Definite>) -> Self {
        let value = value.into();
        self.style.layout.padding.top = value;
        self.style.layout.padding.bottom = value;
        self
    }

    pub fn pt(mut self, value: impl Into<Definite>) -> Self {
        self.style.layout.padding.top = value.into();
        self
    }

    pub fn pr(mut self, value: impl Into<Definite>) -> Self {
        self.style.layout.padding.right = value.into();
        self
    }

    pub fn pb(mut self, value: impl Into<Definite>) -> Self {
        self.style.layout.padding.bottom = value.into();
        self
    }

    pub fn pl(mut self, value: impl Into<Definite>) -> Self {
        self.style.layout.padding.left = value.into();
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

    pub fn mt(mut self, value: impl Into<Length>) -> Self {
        self.style.layout.margin.top = value.into();
        self
    }

    pub fn mr(mut self, value: impl Into<Length>) -> Self {
        self.style.layout.margin.right = value.into();
        self
    }

    pub fn mb(mut self, value: impl Into<Length>) -> Self {
        self.style.layout.margin.bottom = value.into();
        self
    }

    pub fn ml(mut self, value: impl Into<Length>) -> Self {
        self.style.layout.margin.left = value.into();
        self
    }

    pub fn flex_direction(mut self, direction: FlexDirection) -> Self {
        self.style.layout.flex_direction = direction;
        self
    }

    pub fn direction(self, direction: Direction) -> Self {
        self.flex_direction(direction)
    }

    pub fn flex_row(self) -> Self {
        self.flex_direction(FlexDirection::Row)
    }

    pub fn flex(self) -> Self {
        self.display(Display::Flex)
    }

    pub fn block(self) -> Self {
        self.display(Display::Block)
    }

    pub fn flex_col(self) -> Self {
        self.flex_direction(FlexDirection::Column)
    }

    pub fn flex_wrap(mut self, wrap: FlexWrap) -> Self {
        self.style.layout.flex_wrap = wrap;
        self
    }

    pub fn flex_nowrap(self) -> Self {
        self.flex_wrap(FlexWrap::NoWrap)
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

    pub fn flex_1(self) -> Self {
        self.flex_grow(1.0)
            .flex_shrink(1.0)
            .flex_basis(crate::style::Percent(0.0))
    }

    pub fn gap(mut self, gap: impl Into<Gap<Definite>>) -> Self {
        self.style.layout.gap = gap.into();
        self
    }

    pub fn gap_x(mut self, gap: impl Into<Definite>) -> Self {
        self.style.layout.gap.column = gap.into();
        self
    }

    pub fn gap_y(mut self, gap: impl Into<Definite>) -> Self {
        self.style.layout.gap.row = gap.into();
        self
    }

    pub fn justify_content(mut self, justify_content: JustifyContent) -> Self {
        self.style.layout.justify_content = justify_content;
        self
    }

    pub fn justify(self, justify_content: JustifyContent) -> Self {
        self.justify_content(justify_content)
    }

    pub fn justify_center(self) -> Self {
        self.justify_content(JustifyContent::Center)
    }

    pub fn justify_start(self) -> Self {
        self.justify_content(JustifyContent::Start)
    }

    pub fn justify_end(self) -> Self {
        self.justify_content(JustifyContent::End)
    }

    pub fn justify_between(self) -> Self {
        self.justify_content(JustifyContent::SpaceBetween)
    }

    pub fn align_items(mut self, align_items: AlignItems) -> Self {
        self.style.layout.align_items = align_items;
        self
    }

    pub fn align_self(mut self, align_self: AlignSelf) -> Self {
        self.style.layout.align_self = Some(align_self);
        self
    }

    pub fn items_center(self) -> Self {
        self.align_items(AlignItems::Center)
    }

    pub fn items_start(self) -> Self {
        self.align_items(AlignItems::Start)
    }

    pub fn items_end(self) -> Self {
        self.align_items(AlignItems::End)
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

    pub fn display(mut self, display: Display) -> Self {
        self.style.layout.display = display;
        self
    }

    pub fn display_none(self) -> Self {
        self.display(Display::None)
    }

    pub fn hidden(self) -> Self {
        self.display_none()
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

    pub fn background(mut self, background: impl Into<Background>) -> Self {
        self.style.paint.background = Some(background.into());
        self
    }

    pub fn bg(self, background: impl Into<Background>) -> Self {
        self.background(background)
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.style.paint.corner_radii = CornerRadii::all(radius.max(0.0));
        self
    }

    pub fn rounded(self, radius: f32) -> Self {
        self.corner_radius(radius)
    }

    pub fn corner_radii(mut self, corner_radii: CornerRadii) -> Self {
        self.style.paint.corner_radii = CornerRadii {
            top_left: corner_radii.top_left.max(0.0),
            top_right: corner_radii.top_right.max(0.0),
            bottom_right: corner_radii.bottom_right.max(0.0),
            bottom_left: corner_radii.bottom_left.max(0.0),
        };
        self
    }

    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.style.paint.border = Border::all(width, color);
        self
    }

    pub fn border_style(mut self, border: Border) -> Self {
        self.style.paint.border = Border {
            widths: EdgeWidths {
                top: border.widths.top.max(0.0),
                right: border.widths.right.max(0.0),
                bottom: border.widths.bottom.max(0.0),
                left: border.widths.left.max(0.0),
            },
            color: border.color,
        };
        self
    }

    pub fn border_widths(mut self, border_widths: EdgeWidths) -> Self {
        self.style.paint.border.widths = EdgeWidths {
            top: border_widths.top.max(0.0),
            right: border_widths.right.max(0.0),
            bottom: border_widths.bottom.max(0.0),
            left: border_widths.left.max(0.0),
        };
        self
    }

    pub fn border_color(mut self, color: Color) -> Self {
        self.style.paint.border.color = Some(color);
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.paint.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.style.layout.overflow = overflow;
        self
    }

    pub fn overflow_hidden(self) -> Self {
        self.overflow(Overflow::Hidden)
    }

    pub fn overflow_visible(self) -> Self {
        self.overflow(Overflow::Visible)
    }

    pub fn clip(self) -> Self {
        self.overflow_hidden()
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

    pub fn text_color(mut self, color: Color) -> Self {
        self.style.text.color = Some(color);
        self
    }

    pub fn color(self, color: Color) -> Self {
        self.text_color(color)
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

impl ParentElement for Div {
    fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    fn children(mut self, children: impl IntoElements) -> Self {
        children.extend_into(&mut self.children);
        self
    }
}

impl IntoElement for Div {
    fn into_any_element(self) -> AnyElement {
        AnyElement::div(self)
    }
}
