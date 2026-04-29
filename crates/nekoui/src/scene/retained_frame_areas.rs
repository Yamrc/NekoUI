use crate::element::WindowFrameArea;
use crate::scene::LayoutBox;

use super::retained::{NodeId, RetainedTree};

pub(super) fn window_frame_area_at(
    tree: &RetainedTree,
    point: crate::style::Point<crate::style::Px>,
) -> Option<WindowFrameArea> {
    window_frame_area_at_node(tree, tree.root, point, [0.0, 0.0])
}

pub(super) fn collect_window_frame_areas(tree: &RetainedTree) -> Vec<(WindowFrameArea, LayoutBox)> {
    let mut areas = Vec::new();
    collect_window_frame_areas_from(tree, tree.root, [0.0, 0.0], &mut areas);
    areas
}

fn window_frame_area_at_node(
    tree: &RetainedTree,
    node_id: NodeId,
    point: crate::style::Point<crate::style::Px>,
    offset: [f32; 2],
) -> Option<WindowFrameArea> {
    let node = &tree.nodes[node_id];
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
        if let Some(area) = window_frame_area_at_node(tree, child_id, point, child_offset) {
            return Some(area);
        }
    }

    node.window_frame_area
}

fn collect_window_frame_areas_from(
    tree: &RetainedTree,
    node_id: NodeId,
    offset: [f32; 2],
    out: &mut Vec<(WindowFrameArea, LayoutBox)>,
) {
    let node = &tree.nodes[node_id];
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
        collect_window_frame_areas_from(tree, child_id, child_offset, out);
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
