use super::geometry::{Definite, EdgeInsets, Gap, LayoutSize, Length, Px, Size, size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Flex,
    Block,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
}

pub type Direction = FlexDirection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

pub type AlignSelf = AlignItems;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxSizing {
    ContentBox,
    #[default]
    BorderBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutStyle {
    pub display: Display,
    pub size: LayoutSize,
    pub min_size: Size<Option<Definite>>,
    pub max_size: Size<Option<Definite>>,
    pub padding: super::geometry::Edges<Definite>,
    pub margin: super::geometry::Edges<Length>,
    pub flex_direction: FlexDirection,
    pub gap: Gap<Definite>,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_self: Option<AlignSelf>,
    pub flex_wrap: FlexWrap,
    pub flex_basis: Length,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub aspect_ratio: Option<f32>,
    pub box_sizing: BoxSizing,
    pub overflow: Overflow,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            size: LayoutSize::default(),
            min_size: size(None, None),
            max_size: size(None, None),
            padding: EdgeInsets::all(0.0).into(),
            margin: EdgeInsets::all(0.0).into(),
            flex_direction: FlexDirection::default(),
            gap: Gap::all(Definite::from(Px(0.0))),
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            align_self: None,
            flex_wrap: FlexWrap::default(),
            flex_basis: Length::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
            box_sizing: BoxSizing::default(),
            overflow: Overflow::default(),
        }
    }
}
