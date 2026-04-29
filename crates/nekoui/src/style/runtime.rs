use taffy::prelude::{
    AlignItems as TaffyAlignItems, AlignSelf as TaffyAlignSelf, BoxSizing as TaffyBoxSizing,
    Dimension, Display as TaffyDisplay, JustifyContent as TaffyJustifyContent, LengthPercentage,
    LengthPercentageAuto, Rect, Size as TaffySize, Style as TaffyStyle, TaffyAuto,
};
use taffy::style::{FlexDirection as TaffyFlexDirection, Overflow as TaffyOverflow};

use super::{
    Absolute, AlignItems, BoxSizing, Definite, Display, FlexDirection, FlexWrap, JustifyContent,
    Length, Overflow, ResolvedStyle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct StyleChange {
    pub(crate) layout: bool,
    pub(crate) paint: bool,
    pub(crate) text_shape: bool,
}

pub(crate) fn diff_div_style(old: &ResolvedStyle, new: &ResolvedStyle) -> StyleChange {
    StyleChange {
        layout: old.layout != new.layout,
        paint: old.paint != new.paint,
        text_shape: false,
    }
}

pub(crate) fn diff_text_style(old: &ResolvedStyle, new: &ResolvedStyle) -> StyleChange {
    StyleChange {
        layout: old.layout != new.layout,
        paint: old.paint != new.paint || old.text.color != new.text.color,
        text_shape: old.text.font_families != new.text.font_families
            || old.text.font_size != new.text.font_size
            || old.text.line_height != new.text.line_height
            || old.text.font_weight != new.text.font_weight
            || old.text.font_style != new.text.font_style
            || old.text.text_align != new.text.text_align
            || old.text.white_space != new.text.white_space
            || old.text.text_overflow != new.text.text_overflow,
    }
}

pub(crate) fn div_style_to_taffy(style: &ResolvedStyle) -> TaffyStyle {
    TaffyStyle {
        display: match style.layout.display {
            Display::Flex => TaffyDisplay::Flex,
            Display::Block => TaffyDisplay::Block,
            Display::None => TaffyDisplay::None,
        },
        flex_direction: match style.layout.flex_direction {
            FlexDirection::Row => TaffyFlexDirection::Row,
            FlexDirection::Column => TaffyFlexDirection::Column,
        },
        size: TaffySize {
            width: length_to_dimension(style.layout.size.width),
            height: length_to_dimension(style.layout.size.height),
        },
        min_size: TaffySize {
            width: option_definite_to_dimension(style.layout.min_size.width),
            height: option_definite_to_dimension(style.layout.min_size.height),
        },
        max_size: TaffySize {
            width: option_definite_to_dimension(style.layout.max_size.width),
            height: option_definite_to_dimension(style.layout.max_size.height),
        },
        margin: Rect {
            left: edge_to_auto(style.layout.margin.left),
            right: edge_to_auto(style.layout.margin.right),
            top: edge_to_auto(style.layout.margin.top),
            bottom: edge_to_auto(style.layout.margin.bottom),
        },
        padding: Rect {
            left: definite_to_length(style.layout.padding.left),
            right: definite_to_length(style.layout.padding.right),
            top: definite_to_length(style.layout.padding.top),
            bottom: definite_to_length(style.layout.padding.bottom),
        },
        border: Rect {
            left: border_width_to_length(style.paint.border.widths.left),
            right: border_width_to_length(style.paint.border.widths.right),
            top: border_width_to_length(style.paint.border.widths.top),
            bottom: border_width_to_length(style.paint.border.widths.bottom),
        },
        gap: TaffySize {
            width: definite_to_length(style.layout.gap.column),
            height: definite_to_length(style.layout.gap.row),
        },
        align_items: Some(match style.layout.align_items {
            AlignItems::Start => TaffyAlignItems::Start,
            AlignItems::Center => TaffyAlignItems::Center,
            AlignItems::End => TaffyAlignItems::End,
            AlignItems::Stretch => TaffyAlignItems::Stretch,
        }),
        align_self: style.layout.align_self.map(align_self_to_taffy),
        justify_content: Some(match style.layout.justify_content {
            JustifyContent::Start => TaffyJustifyContent::Start,
            JustifyContent::Center => TaffyJustifyContent::Center,
            JustifyContent::End => TaffyJustifyContent::End,
            JustifyContent::SpaceBetween => TaffyJustifyContent::SpaceBetween,
        }),
        flex_wrap: match style.layout.flex_wrap {
            FlexWrap::NoWrap => taffy::style::FlexWrap::NoWrap,
            FlexWrap::Wrap => taffy::style::FlexWrap::Wrap,
        },
        flex_basis: length_to_dimension(style.layout.flex_basis),
        flex_grow: style.layout.flex_grow,
        flex_shrink: style.layout.flex_shrink,
        aspect_ratio: style.layout.aspect_ratio,
        box_sizing: box_sizing_to_taffy(style.layout.box_sizing),
        overflow: taffy::geometry::Point {
            x: overflow_to_taffy(style.layout.overflow),
            y: overflow_to_taffy(style.layout.overflow),
        },
        ..Default::default()
    }
}

pub(crate) fn text_style_to_taffy(style: &ResolvedStyle) -> TaffyStyle {
    TaffyStyle {
        display: TaffyDisplay::Block,
        size: TaffySize {
            width: length_to_dimension(style.layout.size.width),
            height: length_to_dimension(style.layout.size.height),
        },
        min_size: TaffySize {
            width: style
                .layout
                .min_size
                .width
                .map_or(Dimension::length(0.0), definite_to_dimension),
            height: option_definite_to_dimension(style.layout.min_size.height),
        },
        max_size: TaffySize {
            width: option_definite_to_dimension(style.layout.max_size.width),
            height: option_definite_to_dimension(style.layout.max_size.height),
        },
        flex_basis: length_to_dimension(style.layout.flex_basis),
        flex_grow: style.layout.flex_grow,
        flex_shrink: style.layout.flex_shrink,
        aspect_ratio: style.layout.aspect_ratio,
        margin: Rect {
            left: edge_to_auto(style.layout.margin.left),
            right: edge_to_auto(style.layout.margin.right),
            top: edge_to_auto(style.layout.margin.top),
            bottom: edge_to_auto(style.layout.margin.bottom),
        },
        align_self: style.layout.align_self.map(align_self_to_taffy),
        box_sizing: box_sizing_to_taffy(style.layout.box_sizing),
        overflow: taffy::geometry::Point {
            x: overflow_to_taffy(style.layout.overflow),
            y: overflow_to_taffy(style.layout.overflow),
        },
        ..Default::default()
    }
}

fn length_to_dimension(length: Length) -> Dimension {
    match length {
        Length::Auto => Dimension::AUTO,
        Length::Definite(definite) => definite_to_dimension(definite),
        Length::Fill => Dimension::percent(1.0),
    }
}

fn definite_to_dimension(definite: Definite) -> Dimension {
    match definite {
        Definite::Absolute(absolute) => match absolute {
            Absolute::Px(value) => Dimension::length(value.get()),
            Absolute::Rem(value) => Dimension::length(value.get()),
        },
        Definite::Percent(value) => Dimension::percent(value.get()),
    }
}

fn option_definite_to_dimension(value: Option<Definite>) -> Dimension {
    value.map_or(Dimension::AUTO, definite_to_dimension)
}

fn definite_to_length(value: Definite) -> LengthPercentage {
    match value {
        Definite::Absolute(absolute) => match absolute {
            Absolute::Px(value) => LengthPercentage::length(value.get()),
            Absolute::Rem(value) => LengthPercentage::length(value.get()),
        },
        Definite::Percent(value) => LengthPercentage::percent(value.get()),
    }
}

fn border_width_to_length(value: f32) -> LengthPercentage {
    LengthPercentage::length(value.max(0.0))
}

fn edge_to_auto(value: Length) -> LengthPercentageAuto {
    match value {
        Length::Auto => LengthPercentageAuto::auto(),
        Length::Definite(definite) => match definite {
            Definite::Absolute(absolute) => match absolute {
                Absolute::Px(value) => LengthPercentageAuto::length(value.get()),
                Absolute::Rem(value) => LengthPercentageAuto::length(value.get()),
            },
            Definite::Percent(value) => LengthPercentageAuto::percent(value.get()),
        },
        Length::Fill => LengthPercentageAuto::percent(1.0),
    }
}

fn align_self_to_taffy(align_self: crate::style::AlignSelf) -> TaffyAlignSelf {
    match align_self {
        AlignItems::Start => TaffyAlignSelf::Start,
        AlignItems::Center => TaffyAlignSelf::Center,
        AlignItems::End => TaffyAlignSelf::End,
        AlignItems::Stretch => TaffyAlignSelf::Stretch,
    }
}

fn box_sizing_to_taffy(box_sizing: BoxSizing) -> TaffyBoxSizing {
    match box_sizing {
        BoxSizing::ContentBox => TaffyBoxSizing::ContentBox,
        BoxSizing::BorderBox => TaffyBoxSizing::BorderBox,
    }
}

fn overflow_to_taffy(overflow: Overflow) -> TaffyOverflow {
    match overflow {
        Overflow::Visible => TaffyOverflow::Visible,
        Overflow::Hidden => TaffyOverflow::Hidden,
    }
}

#[cfg(test)]
mod tests {
    use crate::style::{Color, ResolvedStyle, px};

    use super::{StyleChange, diff_div_style, diff_text_style};

    #[test]
    fn div_style_diff_classifies_layout_and_paint_separately() {
        let old = ResolvedStyle::default();
        let mut layout = old.clone();
        layout.layout.size.width = px(100.0).into();
        let mut paint = old.clone();
        paint.paint.background = Some(Color::rgb(0x112233).into());

        assert_eq!(
            diff_div_style(&old, &layout),
            StyleChange {
                layout: true,
                paint: false,
                text_shape: false,
            }
        );
        assert_eq!(
            diff_div_style(&old, &paint),
            StyleChange {
                layout: false,
                paint: true,
                text_shape: false,
            }
        );
    }

    #[test]
    fn text_style_diff_marks_text_shape_changes() {
        let old = ResolvedStyle::default();
        let mut new = old.clone();
        new.text.font_size = px(20.0).into();

        assert_eq!(
            diff_text_style(&old, &new),
            StyleChange {
                layout: false,
                paint: false,
                text_shape: true,
            }
        );
    }
}
