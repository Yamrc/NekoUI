use std::sync::Arc;

use slotmap::{SlotMap, new_key_type};
use smallvec::SmallVec;
use taffy::prelude::{
    AlignItems as TaffyAlignItems, AlignSelf as TaffyAlignSelf, AvailableSpace,
    BoxSizing as TaffyBoxSizing, Dimension, Display as TaffyDisplay,
    JustifyContent as TaffyJustifyContent, LengthPercentage, LengthPercentageAuto,
    NodeId as TaffyNodeId, Rect, Size as TaffySize, Style as TaffyStyle, TaffyAuto, TaffyTree,
};
use taffy::style::{FlexDirection as TaffyFlexDirection, Overflow as TaffyOverflow};

use crate::SharedString;
use crate::element::{SpecArena, SpecKind, SpecNode, SpecNodeId, SpecPayload, WindowFrameArea};
use crate::style::{
    Absolute, AlignItems, Background, BoxSizing, Definite, Display, FlexDirection, FlexWrap,
    JustifyContent, Length, Overflow, ResolvedStyle, ResolvedTextStyle,
};
use crate::text_system::{SharedTextLayout, TextMeasureKey, TextSystem, measure_key};
use crate::window::WindowSize;

use super::{
    ClipClass, ClipInfo, ClipShape, CompiledScene, DirtyLaneMask, EffectClass, EffectMask,
    EffectRegion, LayoutBox, LogicalBatch, Primitive, PrimitiveRange, SceneNode, SceneNodeId,
    Transform2D,
};

new_key_type! {
    pub(crate) struct NodeId;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum NodeClass {
    Div,
    Text,
}

#[derive(Debug, Clone)]
pub(crate) struct RetainedNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: SmallVec<[NodeId; 4]>,
    pub kind: NodeKind,
    pub key: Option<u64>,
    pub style: ResolvedStyle,
    pub window_frame_area: Option<WindowFrameArea>,
    pub layout: LayoutBox,
    pub dirty: DirtyLaneMask,
    compiled_fragment: Option<Arc<CompiledSubtreeFragment>>,
    pub taffy_node: TaffyNodeId,
}

#[derive(Debug, Clone)]
pub(crate) enum NodeKind {
    Div,
    Text {
        content: SharedString,
        layout: Option<SharedTextLayout>,
    },
}

#[derive(Debug)]
pub(crate) struct RetainedTree {
    root: NodeId,
    nodes: SlotMap<NodeId, RetainedNode>,
    taffy: TaffyTree<MeasureContext>,
}

#[derive(Debug, Clone)]
enum MeasureContext {
    Text(TextMeasureContext),
}

#[derive(Debug, Clone)]
struct TextMeasureContext {
    text: SharedString,
    style: ResolvedTextStyle,
    last_key: Option<TextMeasureKey>,
    last_layout: Option<SharedTextLayout>,
}

impl TextMeasureContext {
    fn from_spec(spec: &SpecNode) -> Self {
        let text = match &spec.payload {
            SpecPayload::Text(text) => text,
            SpecPayload::None => unreachable!("text measure context requires text payload"),
        };
        Self {
            text: text.clone(),
            style: spec.style.text.clone(),
            last_key: None,
            last_layout: None,
        }
    }
}

#[derive(Debug, Clone)]
struct CompiledSubtreeFragment {
    scene_nodes: Arc<[SceneNode]>,
    primitives: Arc<[Primitive]>,
    logical_batches: Arc<[LogicalBatch]>,
}

impl RetainedTree {
    pub fn from_spec(arena: &SpecArena, root: SpecNodeId) -> Self {
        let mut tree = Self {
            root: NodeId::default(),
            nodes: SlotMap::with_key(),
            taffy: TaffyTree::new(),
        };

        let root_id = tree.build_node(arena, root, None);
        tree.root = root_id;
        tree
    }

    pub fn update_from_spec(&mut self, arena: &SpecArena, root: SpecNodeId) -> DirtyLaneMask {
        self.clear_dirty_marks();

        if !self.can_reuse_node(self.root, arena.node(root)) {
            *self = RetainedTree::from_spec(arena, root);
            return DirtyLaneMask::BUILD.normalized();
        }

        self.diff_node(self.root, arena, root).normalized()
    }

    pub fn compute_layout(&mut self, size: WindowSize, text_system: &mut TextSystem) {
        self.taffy
            .compute_layout_with_measure(
                self.nodes[self.root].taffy_node,
                TaffySize {
                    width: AvailableSpace::Definite(size.width as f32),
                    height: AvailableSpace::Definite(size.height as f32),
                },
                |known_dimensions, available_space, _node_id, context, _style| {
                    let Some(MeasureContext::Text(text_context)) = context else {
                        return taffy::geometry::Size::ZERO;
                    };

                    let width = known_dimensions
                        .width
                        .or_else(|| definite_space(available_space.width));
                    let key = measure_key(&text_context.text, &text_context.style, width);

                    let layout = if let Some(cached_layout) = text_context
                        .last_key
                        .as_ref()
                        .zip(text_context.last_layout.as_ref())
                        .filter(|(cached_key, _)| **cached_key == key)
                        .map(|(_, cached_layout)| cached_layout.clone())
                    {
                        cached_layout
                    } else {
                        text_system.measure(&text_context.text, &text_context.style, width)
                    };

                    text_context.last_key = Some(key);
                    text_context.last_layout = Some(layout.clone());

                    taffy::geometry::Size {
                        width: layout.width,
                        height: layout.height,
                    }
                },
            )
            .expect("taffy layout computation must succeed for retained nodes");

        let taffy = &self.taffy;
        for (node_id, node) in &mut self.nodes {
            debug_assert_eq!(node.id, node_id);

            let layout = *taffy
                .layout(node.taffy_node)
                .expect("layout must be available after compute_layout");
            let next_layout = LayoutBox {
                x: layout.location.x,
                y: layout.location.y,
                width: layout.size.width,
                height: layout.size.height,
            };
            if node.layout != next_layout {
                node.dirty |= DirtyLaneMask::LAYOUT;
            }
            node.layout = next_layout;

            if let NodeKind::Text {
                layout: text_layout,
                ..
            } = &mut node.kind
            {
                *text_layout =
                    taffy
                        .get_node_context(node.taffy_node)
                        .and_then(|context| match context {
                            MeasureContext::Text(text_context) => text_context.last_layout.clone(),
                        });
            }
        }
    }

    #[cfg(test)]
    pub fn root_layout(&self) -> LayoutBox {
        self.nodes[self.root].layout
    }

    #[cfg(test)]
    pub fn root_id(&self) -> NodeId {
        self.root
    }

    #[cfg(test)]
    pub fn node(&self, node_id: NodeId) -> &RetainedNode {
        &self.nodes[node_id]
    }

    #[cfg(test)]
    pub fn children(&self, node_id: NodeId) -> &[NodeId] {
        &self.nodes[node_id].children
    }

    pub fn compile_scene(&mut self) -> CompiledScene {
        let root_fragment = self.get_or_build_subtree_fragment(self.root, false, false);
        let clear_color = self.nodes[self.root]
            .style
            .paint
            .background
            .map(|background| match background {
                Background::Solid(color) => color,
                Background::LinearGradient(gradient) => gradient.start_color,
            });
        CompiledScene {
            clear_color,
            scene_nodes: root_fragment.scene_nodes.clone(),
            primitives: root_fragment.primitives.clone(),
            logical_batches: root_fragment.logical_batches.clone(),
            effect_regions: Arc::from(Vec::<EffectRegion>::new()),
        }
    }

    pub fn window_frame_area_at(
        &self,
        point: crate::style::Point<crate::style::Px>,
    ) -> Option<WindowFrameArea> {
        self.window_frame_area_at_node(self.root, point, [0.0, 0.0])
    }

    pub fn collect_window_frame_areas(&self) -> Vec<(WindowFrameArea, LayoutBox)> {
        let mut areas = Vec::new();
        self.collect_window_frame_areas_from(self.root, [0.0, 0.0], &mut areas);
        areas
    }

    fn clear_dirty_marks(&mut self) {
        for (_, node) in &mut self.nodes {
            node.dirty = DirtyLaneMask::empty();
        }
    }

    fn diff_node(
        &mut self,
        node_id: NodeId,
        arena: &SpecArena,
        spec_id: SpecNodeId,
    ) -> DirtyLaneMask {
        match arena.node(spec_id).kind {
            SpecKind::Div => self.diff_div(node_id, arena, spec_id),
            SpecKind::Text => self.diff_text(node_id, arena, spec_id),
        }
    }

    fn diff_div(
        &mut self,
        node_id: NodeId,
        arena: &SpecArena,
        spec_id: SpecNodeId,
    ) -> DirtyLaneMask {
        let spec = arena.node(spec_id);
        let mut dirty = diff_div_style(&self.nodes[node_id].style, &spec.style);
        if self.nodes[node_id].style != spec.style {
            self.nodes[node_id].style = spec.style.clone();
            self.taffy
                .set_style(
                    self.nodes[node_id].taffy_node,
                    div_style_to_taffy(&spec.style),
                )
                .expect("div style patch must succeed");
        }
        self.nodes[node_id].key = spec.key;
        self.nodes[node_id].window_frame_area = spec.window_frame_area;

        let child_dirty = self.sync_children(node_id, arena, arena.child_ids(spec_id).as_slice());
        dirty |= child_dirty;
        self.nodes[node_id].dirty |= dirty;
        dirty
    }

    fn diff_text(
        &mut self,
        node_id: NodeId,
        arena: &SpecArena,
        spec_id: SpecNodeId,
    ) -> DirtyLaneMask {
        let spec = arena.node(spec_id);
        let text_content = match &spec.payload {
            SpecPayload::Text(content) => content,
            SpecPayload::None => unreachable!("text diff requires text payload"),
        };
        let mut dirty = diff_text_style(&self.nodes[node_id].style, &spec.style);
        let content_changed = match &self.nodes[node_id].kind {
            NodeKind::Text { content, .. } => content.as_ref() != text_content.as_ref(),
            NodeKind::Div => unreachable!("text diff called for non-text node"),
        };

        if content_changed {
            dirty |= DirtyLaneMask::LAYOUT | DirtyLaneMask::PAINT;
        }

        if self.nodes[node_id].style != spec.style {
            self.nodes[node_id].style = spec.style.clone();
            self.taffy
                .set_style(
                    self.nodes[node_id].taffy_node,
                    text_style_to_taffy(&spec.style),
                )
                .expect("text style patch must succeed");
        }

        if let NodeKind::Text { content, layout } = &mut self.nodes[node_id].kind {
            *content = text_content.clone();
            if dirty.needs_layout() || dirty.contains(DirtyLaneMask::PAINT) {
                *layout = None;
            }
        }
        self.nodes[node_id].key = spec.key;
        self.nodes[node_id].window_frame_area = spec.window_frame_area;

        if !dirty.is_empty() {
            self.taffy
                .set_node_context(
                    self.nodes[node_id].taffy_node,
                    Some(MeasureContext::Text(TextMeasureContext::from_spec(spec))),
                )
                .expect("text node context patch must succeed");
        }

        self.nodes[node_id].dirty |= dirty;
        dirty
    }

    fn sync_children(
        &mut self,
        parent_id: NodeId,
        arena: &SpecArena,
        new_children: &[SpecNodeId],
    ) -> DirtyLaneMask {
        let old_children = self.nodes[parent_id].children.clone();
        let keyed_path = self.uses_keyed_path(arena, &old_children, new_children);
        let mut next_children = SmallVec::<[NodeId; 4]>::with_capacity(new_children.len());
        let mut dirty = DirtyLaneMask::empty();
        let mut rebuild_required = false;
        let mut child_list_changed = old_children.len() != new_children.len();
        let mut reused = rustc_hash::FxHashSet::<NodeId>::default();
        let mut keyed_old = rustc_hash::FxHashMap::<(u64, NodeClass), NodeId>::default();

        if keyed_path {
            for child_id in &old_children {
                if let Some(key) = self.nodes[*child_id].key {
                    keyed_old.insert((key, self.node_class(*child_id)), *child_id);
                }
            }
        }

        for (index, spec_id) in new_children.iter().copied().enumerate() {
            let spec = arena.node(spec_id);
            let positional = old_children
                .get(index)
                .copied()
                .filter(|child_id| !reused.contains(child_id));
            let reused_child = if keyed_path {
                if let Some(key) = spec.key {
                    keyed_old
                        .get(&(key, spec_class(spec)))
                        .copied()
                        .filter(|child_id| !reused.contains(child_id))
                        .filter(|child_id| self.can_reuse_node(*child_id, spec))
                        .or_else(|| {
                            positional.filter(|child_id| self.can_reuse_node(*child_id, spec))
                        })
                } else {
                    positional.filter(|child_id| self.can_reuse_node(*child_id, spec))
                }
            } else {
                positional.filter(|child_id| self.can_reuse_node(*child_id, spec))
            };

            let child_id = match reused_child {
                Some(existing_child) => {
                    reused.insert(existing_child);
                    if old_children.get(index).copied() != Some(existing_child) {
                        child_list_changed = true;
                        dirty |= DirtyLaneMask::LAYOUT;
                    }
                    dirty |= self.diff_node(existing_child, arena, spec_id);
                    self.nodes[existing_child].parent = Some(parent_id);
                    existing_child
                }
                None => {
                    child_list_changed = true;
                    rebuild_required = true;
                    dirty |= DirtyLaneMask::BUILD;
                    self.build_node(arena, spec_id, Some(parent_id))
                }
            };

            next_children.push(child_id);
        }

        for old_child in old_children.iter().copied() {
            if !reused.contains(&old_child) {
                child_list_changed = true;
                rebuild_required = true;
                dirty |= DirtyLaneMask::BUILD;
                self.remove_subtree(old_child);
            }
        }

        if child_list_changed {
            self.nodes[parent_id].children = next_children.clone();
            let taffy_children = next_children
                .iter()
                .map(|child_id| self.nodes[*child_id].taffy_node)
                .collect::<SmallVec<[TaffyNodeId; 4]>>();
            self.taffy
                .set_children(self.nodes[parent_id].taffy_node, &taffy_children)
                .expect("children patch must succeed");

            if !rebuild_required {
                dirty |= DirtyLaneMask::LAYOUT;
            }
        }

        dirty
    }

    fn can_reuse_node(&self, node_id: NodeId, spec: &SpecNode) -> bool {
        let node = &self.nodes[node_id];
        let same_kind = matches!(
            (&node.kind, spec.kind),
            (NodeKind::Div, SpecKind::Div) | (NodeKind::Text { .. }, SpecKind::Text)
        );
        if !same_kind {
            return false;
        }

        match (node.key, spec.key) {
            (Some(existing), Some(new)) => existing == new,
            (None, None) => true,
            _ => false,
        }
    }

    fn uses_keyed_path(
        &self,
        arena: &SpecArena,
        old_children: &[NodeId],
        new_children: &[SpecNodeId],
    ) -> bool {
        old_children
            .iter()
            .any(|child_id| self.nodes[*child_id].key.is_some())
            || new_children
                .iter()
                .any(|spec_id| arena.node(*spec_id).key.is_some())
    }

    fn node_class(&self, node_id: NodeId) -> NodeClass {
        match self.nodes[node_id].kind {
            NodeKind::Div => NodeClass::Div,
            NodeKind::Text { .. } => NodeClass::Text,
        }
    }

    fn build_node(
        &mut self,
        arena: &SpecArena,
        spec_id: SpecNodeId,
        parent: Option<NodeId>,
    ) -> NodeId {
        match arena.node(spec_id).kind {
            SpecKind::Div => self.build_div(arena, spec_id, parent),
            SpecKind::Text => self.build_text(arena, spec_id, parent),
        }
    }

    fn build_div(
        &mut self,
        arena: &SpecArena,
        spec_id: SpecNodeId,
        parent: Option<NodeId>,
    ) -> NodeId {
        let spec = arena.node(spec_id);
        let child_ids = arena
            .child_ids(spec_id)
            .into_iter()
            .map(|child| self.build_node(arena, child, None))
            .collect::<SmallVec<[NodeId; 4]>>();
        let child_taffy_nodes = child_ids
            .iter()
            .map(|child_id| self.nodes[*child_id].taffy_node)
            .collect::<SmallVec<[TaffyNodeId; 4]>>();

        let taffy_node = self
            .taffy
            .new_with_children(div_style_to_taffy(&spec.style), &child_taffy_nodes)
            .expect("div node creation must succeed");

        let node_id = self.nodes.insert_with_key(|id| RetainedNode {
            id,
            parent,
            children: child_ids.clone(),
            kind: NodeKind::Div,
            key: spec.key,
            style: spec.style.clone(),
            window_frame_area: spec.window_frame_area,
            layout: LayoutBox::default(),
            dirty: DirtyLaneMask::BUILD.normalized(),
            compiled_fragment: None,
            taffy_node,
        });

        for child_id in child_ids {
            self.nodes[child_id].parent = Some(node_id);
        }

        node_id
    }

    fn build_text(
        &mut self,
        arena: &SpecArena,
        spec_id: SpecNodeId,
        parent: Option<NodeId>,
    ) -> NodeId {
        let spec = arena.node(spec_id);
        let text_content = match &spec.payload {
            SpecPayload::Text(content) => content,
            SpecPayload::None => unreachable!("text node creation requires text payload"),
        };
        let taffy_node = self
            .taffy
            .new_leaf_with_context(
                text_style_to_taffy(&spec.style),
                MeasureContext::Text(TextMeasureContext::from_spec(spec)),
            )
            .expect("text node creation must succeed");

        self.nodes.insert_with_key(|id| RetainedNode {
            id,
            parent,
            children: SmallVec::new(),
            kind: NodeKind::Text {
                content: text_content.clone(),
                layout: None,
            },
            key: spec.key,
            style: spec.style.clone(),
            window_frame_area: spec.window_frame_area,
            layout: LayoutBox::default(),
            dirty: DirtyLaneMask::BUILD.normalized(),
            compiled_fragment: None,
            taffy_node,
        })
    }

    fn remove_subtree(&mut self, node_id: NodeId) {
        let children = self.nodes[node_id].children.clone();
        for child_id in children {
            self.remove_subtree(child_id);
        }

        let taffy_node = self.nodes[node_id].taffy_node;
        self.taffy
            .remove(taffy_node)
            .expect("retained subtree removal must succeed");
        self.nodes.remove(node_id);
    }

    fn window_frame_area_at_node(
        &self,
        node_id: NodeId,
        point: crate::style::Point<crate::style::Px>,
        offset: [f32; 2],
    ) -> Option<WindowFrameArea> {
        let node = &self.nodes[node_id];
        let absolute = LayoutBox {
            x: offset[0] + node.layout.x,
            y: offset[1] + node.layout.y,
            width: node.layout.width,
            height: node.layout.height,
        };

        if !layout_box_contains_point(absolute, point) {
            return None;
        }

        let child_offset = [absolute.x, absolute.y];
        for child_id in node.children.iter().rev().copied() {
            if let Some(area) = self.window_frame_area_at_node(child_id, point, child_offset) {
                return Some(area);
            }
        }

        node.window_frame_area
    }

    fn collect_window_frame_areas_from(
        &self,
        node_id: NodeId,
        offset: [f32; 2],
        out: &mut Vec<(WindowFrameArea, LayoutBox)>,
    ) {
        let node = &self.nodes[node_id];
        let absolute = LayoutBox {
            x: offset[0] + node.layout.x,
            y: offset[1] + node.layout.y,
            width: node.layout.width,
            height: node.layout.height,
        };

        if let Some(area) = node.window_frame_area {
            out.push((area, absolute));
        }

        let child_offset = [absolute.x, absolute.y];
        for child_id in node.children.iter().copied() {
            self.collect_window_frame_areas_from(child_id, child_offset, out);
        }
    }

    fn get_or_build_subtree_fragment(
        &mut self,
        node_id: NodeId,
        ancestor_scene_dirty: bool,
        ancestor_layout_dirty: bool,
    ) -> Arc<CompiledSubtreeFragment> {
        let node = &self.nodes[node_id];
        let subtree_scene_dirty = ancestor_scene_dirty || node.dirty.needs_scene_compile();
        let subtree_layout_dirty = ancestor_layout_dirty
            || node
                .dirty
                .intersects(DirtyLaneMask::BUILD | DirtyLaneMask::LAYOUT);

        if !subtree_scene_dirty && let Some(cached) = &node.compiled_fragment {
            return cached.clone();
        }

        let compiled_fragment =
            if !subtree_layout_dirty && let Some(cached) = node.compiled_fragment.clone() {
                let primitives = self.rebuild_subtree_primitives_only(node_id, subtree_scene_dirty);
                Arc::new(CompiledSubtreeFragment {
                    scene_nodes: cached.scene_nodes.clone(),
                    primitives: Arc::from(primitives),
                    logical_batches: cached.logical_batches.clone(),
                })
            } else {
                Arc::new(self.rebuild_compiled_subtree_fragment(node_id, subtree_scene_dirty))
            };
        self.nodes[node_id].compiled_fragment = Some(compiled_fragment.clone());
        if node_id == self.root {
            self.clear_dirty_after_compile();
        }
        compiled_fragment
    }

    fn rebuild_compiled_subtree_fragment(
        &mut self,
        node_id: NodeId,
        ancestor_scene_dirty: bool,
    ) -> CompiledSubtreeFragment {
        let node = &self.nodes[node_id];
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

        let child_ids = self.nodes[node_id].children.clone();
        let mut first_child = None;
        let mut previous_child: Option<SceneNodeId> = None;
        for child_id in child_ids {
            let child_fragment = self.get_or_build_subtree_fragment(
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
        &mut self,
        node_id: NodeId,
        ancestor_scene_dirty: bool,
    ) -> Vec<Primitive> {
        let node = &self.nodes[node_id];
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

        let child_ids = self.nodes[node_id].children.clone();
        for child_id in child_ids {
            let child_node = &self.nodes[child_id];
            let child_scene_dirty = subtree_scene_dirty || child_node.dirty.needs_scene_compile();
            let child_layout_dirty = child_node
                .dirty
                .intersects(DirtyLaneMask::BUILD | DirtyLaneMask::LAYOUT);

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
                    self.get_or_build_subtree_fragment(child_id, true, true)
                        .primitives
                        .iter()
                        .cloned(),
                );
            } else {
                primitives
                    .extend(self.rebuild_subtree_primitives_only(child_id, subtree_scene_dirty));
            }
        }

        primitives
    }

    fn clear_dirty_after_compile(&mut self) {
        for (_, node) in &mut self.nodes {
            node.dirty = DirtyLaneMask::empty();
        }
    }
}

fn definite_space(space: AvailableSpace) -> Option<f32> {
    match space {
        AvailableSpace::Definite(value) => Some(value),
        AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
    }
}

fn diff_div_style(old: &ResolvedStyle, new: &ResolvedStyle) -> DirtyLaneMask {
    let mut dirty = DirtyLaneMask::empty();
    if old.layout != new.layout {
        dirty |= DirtyLaneMask::LAYOUT;
    }
    if old.paint != new.paint {
        dirty |= DirtyLaneMask::PAINT;
    }
    dirty
}

fn diff_text_style(old: &ResolvedStyle, new: &ResolvedStyle) -> DirtyLaneMask {
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

fn spec_class(spec: &SpecNode) -> NodeClass {
    match spec.kind {
        SpecKind::Div => NodeClass::Div,
        SpecKind::Text => NodeClass::Text,
    }
}

fn div_style_to_taffy(style: &ResolvedStyle) -> TaffyStyle {
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

fn text_style_to_taffy(style: &ResolvedStyle) -> TaffyStyle {
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

fn layout_box_contains_point(
    layout: LayoutBox,
    point: crate::style::Point<crate::style::Px>,
) -> bool {
    let x = point.x.get();
    let y = point.y.get();
    x >= layout.x && x <= layout.x + layout.width && y >= layout.y && y <= layout.y + layout.height
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::app::{App, Render};
    use crate::element::{BuildCx, IntoElement, ParentElement, SpecArena};
    use crate::platform::window::WindowInfoSeed;
    use crate::scene::{Primitive, RectFill};
    use crate::style::px;
    use crate::style::{
        Absolute, BoxSizing, Color, CornerRadii, EdgeInsets, EdgeWidths, FontFamily, TextAlign,
        gradient,
    };
    use crate::text_system::TextSystem;
    use crate::window::{Window, WindowId, WindowInfo, WindowOptions, WindowSize};

    use super::{ClipClass, ClipShape, DirtyLaneMask, EffectClass, NodeKind, RetainedTree};

    fn test_window(logical: WindowSize, physical: WindowSize, scale_factor: f64) -> Window {
        Window::from_options(
            WindowId::new(),
            &WindowOptions::default(),
            WindowInfoSeed {
                content_size: logical,
                frame_size: Some(logical),
                physical_size: physical,
                scale_factor,
                position: None,
                current_display: None,
            },
        )
    }

    fn build_static_tree(root: crate::AnyElement) -> RetainedTree {
        let window = test_window(WindowSize::new(320, 200), WindowSize::new(320, 200), 1.0);
        let mut resolver = |_view_id: u64,
                            _window: &WindowInfo|
         -> Result<crate::AnyElement, crate::RuntimeError> {
            unreachable!("static test tree should not resolve nested views")
        };
        let mut arena = SpecArena::new();
        let built = BuildCx::new(&window, &mut resolver, &mut arena)
            .build_root(root)
            .unwrap();
        RetainedTree::from_spec(&arena, built.root)
    }

    #[test]
    fn text_measurement_wraps_within_available_width() {
        let root = crate::div()
            .width(px(200.0))
            .padding(EdgeInsets::all(10.0))
            .child(crate::text("hello neko ui hello neko ui hello neko ui").font_size(px(16.0)))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(200, 120), &mut text_system);

        let root_layout = tree.root_layout();
        assert_eq!(root_layout.width, 200.0);

        let text_id = tree.children(tree.root_id())[0];
        let text_node = tree.node(text_id);
        assert!(text_node.layout.width <= 180.0 + 0.5);
        assert!(text_node.layout.height > 20.0);

        match &text_node.kind {
            NodeKind::Text { layout, .. } => {
                let layout = layout.as_ref().expect("text layout exists");
                assert!(layout.runs.len() >= 2);
            }
            NodeKind::Div => panic!("expected text node"),
        }
    }

    #[test]
    fn text_style_inherits_from_parent_div_and_child_can_override() {
        let inherited_color = Color::rgb(0x112233);
        let override_color = Color::rgb(0x445566);
        let root = crate::div()
            .font_family(["Noto Sans SC", "Segoe UI Emoji"])
            .font_size(px(24.0))
            .text_color(inherited_color)
            .text_center()
            .child(crate::text("inherited"))
            .child(crate::text("override").color(override_color))
            .into_any_element();

        let tree = build_static_tree(root);
        let first = tree.children(tree.root_id())[0];
        let second = tree.children(tree.root_id())[1];

        assert_eq!(
            tree.node(first).style.text.font_families.as_ref(),
            &[
                FontFamily::from("Noto Sans SC"),
                FontFamily::from("Segoe UI Emoji")
            ]
        );
        assert_eq!(
            tree.node(first).style.text.font_size,
            Absolute::from(px(24.0))
        );
        assert_eq!(tree.node(first).style.text.color, inherited_color);
        assert_eq!(tree.node(first).style.text.text_align, TextAlign::Center);
        assert_eq!(tree.node(second).style.text.color, override_color);
    }

    #[test]
    fn box_and_paint_style_do_not_inherit_to_text_children() {
        let root = crate::div()
            .bg(Color::rgb(0x111111))
            .border(2.0, Color::rgb(0x222222))
            .opacity(0.25)
            .child(crate::text("child"))
            .into_any_element();

        let tree = build_static_tree(root);
        let child = tree.children(tree.root_id())[0];
        assert_eq!(tree.node(child).style.paint.background, None);
        assert_eq!(
            tree.node(child).style.paint.border.widths,
            EdgeWidths::default()
        );
        assert_eq!(tree.node(child).style.paint.border.color, None);
        assert_eq!(tree.node(child).style.paint.opacity, 1.0);
    }

    #[test]
    fn align_self_overrides_parent_cross_axis_alignment() {
        let root = crate::div()
            .size(crate::style::size(px(200.0).into(), px(100.0).into()))
            .items_start()
            .child(crate::div().w(px(20.0)).h(px(20.0)).self_end())
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);

        let child = tree.children(tree.root_id())[0];
        assert_eq!(tree.node(child).layout.y, 80.0);
    }

    #[test]
    fn box_sizing_controls_whether_border_and_padding_expand_layout_size() {
        fn root_width_for(box_sizing: BoxSizing) -> f32 {
            let root = crate::div()
                .box_sizing(box_sizing)
                .w(px(100.0))
                .p(px(10.0))
                .border(5.0, Color::rgb(0x222222))
                .into_any_element();
            let mut tree = build_static_tree(root);
            let mut text_system = TextSystem::new();
            tree.compute_layout(WindowSize::new(500, 200), &mut text_system);
            tree.root_layout().width
        }

        assert_eq!(root_width_for(BoxSizing::BorderBox), 100.0);
        assert_eq!(root_width_for(BoxSizing::ContentBox), 130.0);
    }

    #[test]
    fn diff_marks_paint_without_rebuilding_tree_for_text_color_change() {
        let root = crate::div().child(crate::text("hello").color(Color::rgb(0x111111)));
        let updated = crate::div().child(crate::text("hello").color(Color::rgb(0x222222)));

        let mut tree = build_static_tree(root.into_any_element());
        let window = test_window(WindowSize::new(320, 200), WindowSize::new(320, 200), 1.0);
        let mut resolver = |_view_id: u64,
                            _window: &WindowInfo|
         -> Result<crate::AnyElement, crate::RuntimeError> {
            unreachable!("static test tree should not resolve nested views")
        };
        let mut arena = SpecArena::new();
        let built = BuildCx::new(&window, &mut resolver, &mut arena)
            .build_root(updated.into_any_element())
            .unwrap();
        let dirty = tree.update_from_spec(&arena, built.root);
        assert_eq!(dirty, DirtyLaneMask::PAINT);
    }

    #[test]
    fn diff_marks_layout_for_div_size_change() {
        let root = crate::div().width(px(100.0));
        let updated = crate::div().width(px(140.0));

        let mut tree = build_static_tree(root.into_any_element());
        let window = test_window(WindowSize::new(320, 200), WindowSize::new(320, 200), 1.0);
        let mut resolver = |_view_id: u64,
                            _window: &WindowInfo|
         -> Result<crate::AnyElement, crate::RuntimeError> {
            unreachable!("static test tree should not resolve nested views")
        };
        let mut arena = SpecArena::new();
        let built = BuildCx::new(&window, &mut resolver, &mut arena)
            .build_root(updated.into_any_element())
            .unwrap();
        let dirty = tree.update_from_spec(&arena, built.root);
        assert_eq!(dirty, DirtyLaneMask::LAYOUT);
    }

    #[test]
    fn diff_marks_build_for_child_structure_change() {
        let root = crate::div().child(crate::text("a"));
        let updated = crate::div().child(crate::text("a")).child(crate::text("b"));

        let mut tree = build_static_tree(root.into_any_element());
        let window = test_window(WindowSize::new(320, 200), WindowSize::new(320, 200), 1.0);
        let mut resolver = |_view_id: u64,
                            _window: &WindowInfo|
         -> Result<crate::AnyElement, crate::RuntimeError> {
            unreachable!("static test tree should not resolve nested views")
        };
        let mut arena = SpecArena::new();
        let built = BuildCx::new(&window, &mut resolver, &mut arena)
            .build_root(updated.into_any_element())
            .unwrap();
        let dirty = tree.update_from_spec(&arena, built.root);
        assert_eq!(dirty, DirtyLaneMask::BUILD.normalized());
    }

    #[test]
    fn keyed_reorder_reuses_existing_nodes_without_build() {
        let root = crate::div()
            .child(crate::text("a").key(1))
            .child(crate::text("b").key(2));
        let updated = crate::div()
            .child(crate::text("b").key(2))
            .child(crate::text("a").key(1));

        let mut tree = build_static_tree(root.into_any_element());
        let first = tree.children(tree.root_id())[0];
        let second = tree.children(tree.root_id())[1];

        let window = test_window(WindowSize::new(320, 200), WindowSize::new(320, 200), 1.0);
        let mut resolver = |_view_id: u64,
                            _window: &WindowInfo|
         -> Result<crate::AnyElement, crate::RuntimeError> {
            unreachable!("static test tree should not resolve nested views")
        };
        let mut arena = SpecArena::new();
        let built = BuildCx::new(&window, &mut resolver, &mut arena)
            .build_root(updated.into_any_element())
            .unwrap();
        let dirty = tree.update_from_spec(&arena, built.root);
        assert!(!dirty.contains(DirtyLaneMask::BUILD));
        assert!(dirty.contains(DirtyLaneMask::LAYOUT));
        assert_eq!(tree.children(tree.root_id()), &[second, first]);
    }

    #[test]
    fn compile_scene_emits_scene_nodes_and_logical_batches() {
        let root = crate::div()
            .bg(Color::rgb(0x111111))
            .child(crate::text("hello"))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let compiled = tree.compile_scene();

        assert_eq!(compiled.scene_nodes.len(), 2);
        assert!(!compiled.primitives.is_empty());
        assert!(!compiled.logical_batches.is_empty());
        assert_eq!(compiled.logical_batches[0].primitive_range.start, 0);
        assert!(
            compiled.logical_batches.last().unwrap().primitive_range.end as usize
                <= compiled.primitives.len()
        );
    }

    #[test]
    fn compile_scene_carries_clip_and_opacity_metadata() {
        let root = crate::div()
            .clip()
            .opacity(0.5)
            .child(crate::text("hello").opacity(0.25))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let compiled = tree.compile_scene();

        assert_eq!(compiled.scene_nodes[0].opacity, 0.5);
        assert!(compiled.scene_nodes[0].clip.shape.is_some());
        assert_eq!(compiled.scene_nodes[1].opacity, 0.25);
    }

    #[test]
    fn paint_only_update_preserves_logical_batch_structure() {
        let root = crate::div()
            .bg(Color::rgb(0x111111))
            .child(crate::text("hello").color(Color::rgb(0x222222)))
            .into_any_element();
        let updated = crate::div()
            .bg(Color::rgb(0x333333))
            .child(crate::text("hello").color(Color::rgb(0x444444)))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let original = tree.compile_scene();

        let window = test_window(WindowSize::new(320, 200), WindowSize::new(320, 200), 1.0);
        let mut resolver = |_view_id: u64,
                            _window: &WindowInfo|
         -> Result<crate::AnyElement, crate::RuntimeError> {
            unreachable!("static test tree should not resolve nested views")
        };
        let mut arena = SpecArena::new();
        let built = BuildCx::new(&window, &mut resolver, &mut arena)
            .build_root(updated)
            .unwrap();
        let dirty = tree.update_from_spec(&arena, built.root);
        assert_eq!(dirty, DirtyLaneMask::PAINT);
        let updated_scene = tree.compile_scene();

        assert_eq!(original.scene_nodes.len(), updated_scene.scene_nodes.len());
        assert_eq!(
            original.logical_batches.len(),
            updated_scene.logical_batches.len()
        );
        assert_eq!(
            original.logical_batches[0].primitive_range,
            updated_scene.logical_batches[0].primitive_range
        );
    }

    #[test]
    fn paint_only_update_reuses_root_scene_structure_cache() {
        let root = crate::div()
            .bg(Color::rgb(0x111111))
            .child(crate::text("hello").color(Color::rgb(0x222222)))
            .into_any_element();
        let updated = crate::div()
            .bg(Color::rgb(0x333333))
            .child(crate::text("hello").color(Color::rgb(0x444444)))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let original = tree.compile_scene();

        let window = test_window(WindowSize::new(320, 200), WindowSize::new(320, 200), 1.0);
        let mut resolver = |_view_id: u64,
                            _window: &WindowInfo|
         -> Result<crate::AnyElement, crate::RuntimeError> {
            unreachable!("static test tree should not resolve nested views")
        };
        let mut arena = SpecArena::new();
        let built = BuildCx::new(&window, &mut resolver, &mut arena)
            .build_root(updated)
            .unwrap();
        let dirty = tree.update_from_spec(&arena, built.root);
        assert_eq!(dirty, DirtyLaneMask::PAINT);

        let updated_scene = tree.compile_scene();
        assert!(Arc::ptr_eq(
            &original.scene_nodes,
            &updated_scene.scene_nodes
        ));
        assert!(Arc::ptr_eq(
            &original.logical_batches,
            &updated_scene.logical_batches
        ));
        assert!(!Arc::ptr_eq(
            &original.primitives,
            &updated_scene.primitives
        ));
    }

    #[test]
    fn logical_batches_reflect_clip_and_opacity_classes() {
        let root = crate::div()
            .clip()
            .opacity(0.5)
            .bg(Color::rgb(0x111111))
            .child(crate::text("hello"))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let compiled = tree.compile_scene();

        assert!(
            compiled
                .logical_batches
                .iter()
                .any(|batch| batch.clip_class == ClipClass::Rect)
        );
        assert!(
            compiled
                .logical_batches
                .iter()
                .any(|batch| batch.effect_class == EffectClass::Opacity)
        );
    }

    #[test]
    fn rounded_overflow_hidden_compiles_to_rounded_clip_shape() {
        let root = crate::div()
            .clip()
            .corner_radii(CornerRadii::all(12.0))
            .child(crate::text("hello"))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let compiled = tree.compile_scene();

        assert!(matches!(
            compiled.scene_nodes[0].clip.shape,
            Some(ClipShape::RoundedRect { .. })
        ));
        assert!(
            compiled
                .logical_batches
                .iter()
                .any(|batch| batch.clip_class == ClipClass::RoundedRect)
        );
    }

    #[test]
    fn compile_scene_rect_primitive_carries_corner_and_border_style() {
        let root = crate::div()
            .bg(Color::rgb(0x111111))
            .corner_radii(CornerRadii {
                top_left: 8.0,
                top_right: 12.0,
                bottom_right: 16.0,
                bottom_left: 20.0,
            })
            .border_widths(EdgeWidths {
                top: 1.0,
                right: 2.0,
                bottom: 3.0,
                left: 4.0,
            })
            .border_color(Color::rgb(0x222222))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let compiled = tree.compile_scene();

        let Primitive::Rect(rect) = &compiled.primitives[0] else {
            panic!("expected rect primitive");
        };
        assert_eq!(rect.corner_radii.top_left, 8.0);
        assert_eq!(rect.corner_radii.top_right, 12.0);
        assert_eq!(rect.corner_radii.bottom_right, 16.0);
        assert_eq!(rect.corner_radii.bottom_left, 20.0);
        assert_eq!(rect.border_widths.top, 1.0);
        assert_eq!(rect.border_widths.right, 2.0);
        assert_eq!(rect.border_widths.bottom, 3.0);
        assert_eq!(rect.border_widths.left, 4.0);
        assert_eq!(rect.border_color, Some(Color::rgb(0x222222)));
    }

    #[test]
    fn compile_scene_emits_rect_for_border_only_div() {
        let root = crate::div()
            .border(2.0, Color::rgb(0x333333))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let compiled = tree.compile_scene();

        assert!(!compiled.primitives.is_empty());
        let Primitive::Rect(rect) = &compiled.primitives[0] else {
            panic!("expected rect primitive");
        };
        assert_eq!(rect.fill, RectFill::Solid(Color::rgba(0.0, 0.0, 0.0, 0.0)));
    }

    #[test]
    fn compile_scene_rect_primitive_carries_gradient_fill() {
        let root = crate::div()
            .bg(gradient(Color::rgb(0x111111), Color::rgb(0x444444), 1.25))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let compiled = tree.compile_scene();

        let Primitive::Rect(rect) = &compiled.primitives[0] else {
            panic!("expected rect primitive");
        };
        let RectFill::LinearGradient(gradient) = rect.fill else {
            panic!("expected gradient fill");
        };
        assert_eq!(gradient.start_color, Color::rgb(0x111111));
        assert_eq!(gradient.end_color, Color::rgb(0x444444));
        assert_eq!(gradient.angle_radians, 1.25);
    }

    #[test]
    fn paint_only_update_reuses_clean_sibling_fragment_cache() {
        let root = crate::div()
            .child(crate::div().bg(Color::rgb(0x111111)).key(1))
            .child(crate::div().bg(Color::rgb(0x222222)).key(2))
            .into_any_element();
        let updated = crate::div()
            .child(crate::div().bg(Color::rgb(0x333333)).key(1))
            .child(crate::div().bg(Color::rgb(0x222222)).key(2))
            .into_any_element();

        let mut tree = build_static_tree(root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(320, 200), &mut text_system);
        let _ = tree.compile_scene();

        let sibling = tree.children(tree.root_id())[1];
        let original_fragment = tree.nodes[sibling]
            .compiled_fragment
            .clone()
            .expect("clean sibling fragment cached after compile");

        let window = test_window(WindowSize::new(320, 200), WindowSize::new(320, 200), 1.0);
        let mut resolver = |_view_id: u64,
                            _window: &WindowInfo|
         -> Result<crate::AnyElement, crate::RuntimeError> {
            unreachable!("static test tree should not resolve nested views")
        };
        let mut arena = SpecArena::new();
        let built = BuildCx::new(&window, &mut resolver, &mut arena)
            .build_root(updated)
            .unwrap();
        let dirty = tree.update_from_spec(&arena, built.root);
        assert_eq!(dirty, DirtyLaneMask::PAINT);

        let _ = tree.compile_scene();
        let reused_fragment = tree.nodes[sibling]
            .compiled_fragment
            .clone()
            .expect("clean sibling fragment remains cached");
        assert!(Arc::ptr_eq(&original_fragment, &reused_fragment));
    }

    #[test]
    fn view_nodes_resolve_before_retained_layout() {
        struct LabelView;

        impl Render for LabelView {
            fn render(
                &mut self,
                _window: &WindowInfo,
                _cx: &mut crate::Context<'_, Self>,
            ) -> impl IntoElement {
                crate::text("resolved from view")
            }
        }

        let app = App::new(Vec::new());
        let view = app.insert_view(LabelView);
        let window = test_window(WindowSize::new(320, 200), WindowSize::new(640, 400), 2.0);
        let root = crate::div().child(view).into_any_element();
        let mut arena = SpecArena::new();
        let built = app.build_root_spec(&window, &root, &mut arena).unwrap();
        let mut tree = RetainedTree::from_spec(&arena, built.root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(window.content_size(), &mut text_system);

        let child = tree.children(tree.root_id())[0];
        assert!(matches!(tree.node(child).kind, NodeKind::Text { .. }));
    }
}
