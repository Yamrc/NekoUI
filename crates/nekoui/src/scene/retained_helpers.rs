use taffy::prelude::{
    AlignItems as TaffyAlignItems, AlignSelf as TaffyAlignSelf, AvailableSpace,
    BoxSizing as TaffyBoxSizing, Dimension, Display as TaffyDisplay,
    JustifyContent as TaffyJustifyContent, LengthPercentage, LengthPercentageAuto, Rect,
    Size as TaffySize, Style as TaffyStyle, TaffyAuto,
};
use taffy::style::{FlexDirection as TaffyFlexDirection, Overflow as TaffyOverflow};

use crate::element::{SpecKind, SpecNode};
use crate::scene::{
    ClipClass, ClipShape, EffectClass, LayoutBox, LogicalBatch, Primitive, PrimitiveRange,
    SceneNode, SceneNodeId,
};
use crate::style::{
    Absolute, AlignItems, BoxSizing, Definite, Display, FlexDirection, FlexWrap, JustifyContent,
    Length, Overflow, ResolvedStyle,
};

use super::dirty::DirtyLaneMask;
use super::retained::{CompiledSubtreeFragment, NodeClass};

pub(super) fn definite_space(space: AvailableSpace) -> Option<f32> {
    match space {
        AvailableSpace::Definite(value) => Some(value),
        AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
    }
}

pub(super) fn diff_div_style(old: &ResolvedStyle, new: &ResolvedStyle) -> DirtyLaneMask {
    let mut dirty = DirtyLaneMask::empty();
    if old.layout != new.layout {
        dirty |= DirtyLaneMask::LAYOUT;
    }
    if old.paint != new.paint {
        dirty |= DirtyLaneMask::PAINT;
    }
    dirty
}

pub(super) fn diff_text_style(old: &ResolvedStyle, new: &ResolvedStyle) -> DirtyLaneMask {
    let mut dirty = DirtyLaneMask::empty();
    if old.layout != new.layout {
        dirty |= DirtyLaneMask::LAYOUT;
    }
    if old.paint != new.paint || old.text.color != new.text.color {
        dirty |= DirtyLaneMask::PAINT;
    }
    if old.text.font_families != new.text.font_families
        || old.text.font_size != new.text.font_size
        || old.text.line_height != new.text.line_height
        || old.text.font_weight != new.text.font_weight
        || old.text.font_style != new.text.font_style
        || old.text.text_align != new.text.text_align
        || old.text.white_space != new.text.white_space
        || old.text.text_overflow != new.text.text_overflow
    {
        dirty |= DirtyLaneMask::LAYOUT | DirtyLaneMask::PAINT;
    }
    dirty
}

pub(super) fn spec_class(spec: &SpecNode) -> NodeClass {
    match spec.kind {
        SpecKind::Div => NodeClass::Div,
        SpecKind::Text => NodeClass::Text,
    }
}

pub(super) fn div_style_to_taffy(style: &ResolvedStyle) -> TaffyStyle {
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

pub(super) fn text_style_to_taffy(style: &ResolvedStyle) -> TaffyStyle {
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

pub(super) fn layout_box_contains_point(
    layout: LayoutBox,
    point: crate::style::Point<crate::style::Px>,
) -> bool {
    let x = point.x.get();
    let y = point.y.get();
    x >= layout.x && x <= layout.x + layout.width && y >= layout.y && y <= layout.y + layout.height
}

pub(super) fn build_logical_batches(
    scene_nodes: &[SceneNode],
    primitives: &[Primitive],
) -> Vec<LogicalBatch> {
    let mut batches: Vec<LogicalBatch> = Vec::new();
    let batch_meta = primitive_batch_meta(scene_nodes, primitives.len());

    for (index, primitive) in primitives.iter().enumerate() {
        let material_class = primitive.material_class();
        let (clip_class, effect_class) = batch_meta[index];
        if let Some(last_batch) = batches.last_mut()
            && last_batch.material_class == material_class
            && last_batch.clip_class == clip_class
            && last_batch.effect_class == effect_class
            && last_batch.primitive_range.end == index as u32
        {
            last_batch.primitive_range.end += 1;
            continue;
        }

        batches.push(LogicalBatch {
            primitive_range: PrimitiveRange::new(index as u32, index as u32 + 1),
            material_class,
            clip_class,
            effect_class,
        });
    }

    batches
}

fn primitive_batch_meta(
    scene_nodes: &[SceneNode],
    primitive_count: usize,
) -> Vec<(ClipClass, EffectClass)> {
    let mut out = vec![(ClipClass::None, EffectClass::None); primitive_count];
    if !scene_nodes.is_empty() {
        assign_batch_meta(
            scene_nodes,
            SceneNodeId(0),
            ClipClass::None,
            EffectClass::None,
            &mut out,
        );
    }
    out
}

fn assign_batch_meta(
    scene_nodes: &[SceneNode],
    node_id: SceneNodeId,
    parent_clip: ClipClass,
    parent_effect: EffectClass,
    out: &mut [(ClipClass, EffectClass)],
) {
    let node = &scene_nodes[node_id.0 as usize];
    let clip_class = if let Some(shape) = node.clip.shape {
        shape.class()
    } else {
        parent_clip
    };
    let effect_class = if (node.opacity - 1.0).abs() > f32::EPSILON {
        EffectClass::Opacity
    } else {
        parent_effect
    };

    for primitive_index in node.primitive_range.as_range() {
        out[primitive_index] = (clip_class, effect_class);
    }

    let mut child = node.first_child;
    while let Some(child_id) = child {
        assign_batch_meta(scene_nodes, child_id, clip_class, effect_class, out);
        child = scene_nodes[child_id.0 as usize].next_sibling;
    }
}

pub(super) fn clip_shape_for_node(
    overflow: Overflow,
    bounds: LayoutBox,
    paint: &crate::style::PaintStyle,
) -> Option<ClipShape> {
    if overflow != Overflow::Hidden {
        return None;
    }

    if has_non_zero_corner_radii(paint.corner_radii) {
        return Some(ClipShape::RoundedRect {
            bounds,
            corner_radii: paint.corner_radii,
        });
    }

    Some(ClipShape::Rect(bounds))
}

fn has_non_zero_corner_radii(corner_radii: crate::style::CornerRadii) -> bool {
    corner_radii.top_left > 0.0
        || corner_radii.top_right > 0.0
        || corner_radii.bottom_right > 0.0
        || corner_radii.bottom_left > 0.0
}

pub(super) fn append_subtree_fragment(
    fragment: &CompiledSubtreeFragment,
    parent: Option<SceneNodeId>,
    out_nodes: &mut Vec<SceneNode>,
    out_primitives: &mut Vec<Primitive>,
) -> SceneNodeId {
    let node_offset = out_nodes.len() as u32;
    let primitive_offset = out_primitives.len() as u32;

    for (index, node) in fragment.scene_nodes.iter().enumerate() {
        let local_scene_id = SceneNodeId(index as u32);
        out_nodes.push(SceneNode {
            parent: if local_scene_id.0 == 0 {
                parent
            } else {
                node.parent.map(|id| SceneNodeId(id.0 + node_offset))
            },
            first_child: node.first_child.map(|id| SceneNodeId(id.0 + node_offset)),
            next_sibling: node.next_sibling.map(|id| SceneNodeId(id.0 + node_offset)),
            transform: node.transform,
            clip: node.clip,
            opacity: node.opacity,
            effect_mask: node.effect_mask,
            primitive_range: PrimitiveRange::new(
                node.primitive_range.start + primitive_offset,
                node.primitive_range.end + primitive_offset,
            ),
        });
    }

    out_primitives.extend(fragment.primitives.iter().cloned());
    SceneNodeId(node_offset)
}
