use std::sync::Arc;

use crate::scene::{
    ClipClass, ClipShape, EffectClass, LayoutBox, LogicalBatch, Primitive, PrimitiveRange,
    SceneNode, SceneNodeId,
};
use crate::style::Overflow;

use super::primitive::EffectMask;
use super::retained::{CompiledSubtreeFragment, NodeId, NodeKind, RetainedTree};
use super::{ClipInfo, Transform2D};

pub(super) fn get_or_build_subtree_fragment(
    tree: &mut RetainedTree,
    node_id: NodeId,
    ancestor_scene_dirty: bool,
    ancestor_layout_dirty: bool,
) -> Arc<CompiledSubtreeFragment> {
    let node = &tree.nodes[node_id];
    let subtree_scene_dirty = ancestor_scene_dirty || node.dirty.needs_scene_compile();
    let subtree_layout_dirty = ancestor_layout_dirty
        || node
            .dirty
            .intersects(super::DirtyLaneMask::BUILD | super::DirtyLaneMask::LAYOUT);

    if !subtree_scene_dirty && let Some(cached) = &node.compiled_fragment {
        return cached.clone();
    }

    let compiled_fragment =
        if !subtree_layout_dirty && let Some(cached) = node.compiled_fragment.clone() {
            let primitives = rebuild_subtree_primitives_only(tree, node_id, subtree_scene_dirty);
            Arc::new(CompiledSubtreeFragment {
                scene_nodes: cached.scene_nodes.clone(),
                primitives: Arc::from(primitives),
                logical_batches: cached.logical_batches.clone(),
            })
        } else {
            Arc::new(rebuild_compiled_subtree_fragment(
                tree,
                node_id,
                subtree_scene_dirty,
            ))
        };
    tree.nodes[node_id].compiled_fragment = Some(compiled_fragment.clone());
    if node_id == tree.root {
        clear_dirty_after_compile(tree);
    }
    compiled_fragment
}

fn rebuild_compiled_subtree_fragment(
    tree: &mut RetainedTree,
    node_id: NodeId,
    ancestor_scene_dirty: bool,
) -> CompiledSubtreeFragment {
    let node = &tree.nodes[node_id];
    debug_assert_eq!(node.id, node_id);
    let subtree_scene_dirty = ancestor_scene_dirty || node.dirty.needs_scene_compile();

    let bounds = LayoutBox {
        x: 0.0,
        y: 0.0,
        width: node.layout.width,
        height: node.layout.height,
    };

    let mut scene_nodes = Vec::with_capacity(node.children.len() + 1);
    let mut primitives = Vec::with_capacity(node.children.len() + 1);
    scene_nodes.push(SceneNode {
        parent: None,
        first_child: None,
        next_sibling: None,
        transform: Transform2D {
            tx: node.layout.x,
            ty: node.layout.y,
        },
        clip: ClipInfo {
            shape: clip_shape_for_node(node.style.layout.overflow, bounds, &node.style.paint),
        },
        opacity: node.style.paint.opacity,
        effect_mask: EffectMask::default(),
        primitive_range: PrimitiveRange::default(),
    });

    match &node.kind {
        NodeKind::Div => {
            if let Some(rect) = super::RectPrimitive::from_paint(bounds, &node.style.paint) {
                primitives.push(Primitive::Rect(rect));
            }
        }
        NodeKind::Text { layout, .. } => {
            if let Some(layout) = layout.clone() {
                primitives.push(Primitive::Text {
                    bounds,
                    layout,
                    color: node.style.text.color,
                });
            }
        }
    }
    scene_nodes[0].primitive_range = PrimitiveRange::new(0, primitives.len() as u32);

    let child_ids = tree.nodes[node_id].children.clone();
    let mut first_child = None;
    let mut previous_child: Option<SceneNodeId> = None;
    for child_id in child_ids {
        let child_fragment = get_or_build_subtree_fragment(
            tree,
            child_id,
            subtree_scene_dirty || ancestor_scene_dirty,
            true,
        );
        let child_root = append_subtree_fragment(
            &child_fragment,
            Some(SceneNodeId(0)),
            &mut scene_nodes,
            &mut primitives,
        );
        if first_child.is_none() {
            first_child = Some(child_root);
        }
        if let Some(previous_child) = previous_child {
            scene_nodes[previous_child.0 as usize].next_sibling = Some(child_root);
        }
        previous_child = Some(child_root);
    }
    scene_nodes[0].first_child = first_child;

    let logical_batches = build_logical_batches(&scene_nodes, &primitives);
    CompiledSubtreeFragment {
        scene_nodes: Arc::from(scene_nodes),
        primitives: Arc::from(primitives),
        logical_batches: Arc::from(logical_batches),
    }
}

fn rebuild_subtree_primitives_only(
    tree: &mut RetainedTree,
    node_id: NodeId,
    ancestor_scene_dirty: bool,
) -> Vec<Primitive> {
    let node = &tree.nodes[node_id];
    let subtree_scene_dirty = ancestor_scene_dirty || node.dirty.needs_scene_compile();

    if !subtree_scene_dirty && node.compiled_fragment.is_some() {
        return node
            .compiled_fragment
            .as_ref()
            .map(|f| f.primitives.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
    }

    let bounds = LayoutBox {
        x: 0.0,
        y: 0.0,
        width: node.layout.width,
        height: node.layout.height,
    };

    let mut primitives = Vec::with_capacity(node.children.len() + 1);

    match &node.kind {
        NodeKind::Div => {
            if let Some(rect) = super::RectPrimitive::from_paint(bounds, &node.style.paint) {
                primitives.push(Primitive::Rect(rect));
            }
        }
        NodeKind::Text { layout, .. } => {
            if let Some(layout) = layout.clone() {
                primitives.push(Primitive::Text {
                    bounds,
                    layout,
                    color: node.style.text.color,
                });
            }
        }
    }

    let child_ids = tree.nodes[node_id].children.clone();
    for child_id in child_ids {
        let child_node = &tree.nodes[child_id];
        let child_scene_dirty = subtree_scene_dirty || child_node.dirty.needs_scene_compile();
        let child_layout_dirty = child_node
            .dirty
            .intersects(super::DirtyLaneMask::BUILD | super::DirtyLaneMask::LAYOUT);

        if !child_scene_dirty && child_node.compiled_fragment.is_some() {
            primitives.extend(
                child_node
                    .compiled_fragment
                    .as_ref()
                    .map(|f| f.primitives.iter().cloned().collect::<Vec<_>>())
                    .unwrap_or_default(),
            );
            continue;
        }

        if child_layout_dirty {
            primitives.extend(
                get_or_build_subtree_fragment(tree, child_id, true, true)
                    .primitives
                    .iter()
                    .cloned(),
            );
        } else {
            primitives.extend(rebuild_subtree_primitives_only(
                tree,
                child_id,
                subtree_scene_dirty,
            ));
        }
    }

    primitives
}

fn clear_dirty_after_compile(tree: &mut RetainedTree) {
    for (_, node) in &mut tree.nodes {
        node.dirty = super::DirtyLaneMask::empty();
    }
}

fn build_logical_batches(scene_nodes: &[SceneNode], primitives: &[Primitive]) -> Vec<LogicalBatch> {
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

fn clip_shape_for_node(
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

fn append_subtree_fragment(
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
