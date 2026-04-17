use std::collections::HashMap;

use slotmap::{SlotMap, new_key_type};
use taffy::prelude::{
    AlignItems as TaffyAlignItems, AvailableSpace, Dimension, Display,
    JustifyContent as TaffyJustifyContent, LengthPercentage, LengthPercentageAuto,
    NodeId as TaffyNodeId, Rect, Size as TaffySize, Style as TaffyStyle, TaffyAuto, TaffyTree,
};
use taffy::style::FlexDirection as TaffyFlexDirection;

use crate::SharedString;
use crate::element::{Div, Element, ElementKind, Text};
use crate::style::{AlignItems, Direction, JustifyContent, Length, Style};
use crate::text_system::{TextLayout, TextMeasureKey, TextSystem, measure_key};
use crate::window::WindowSize;

new_key_type! {
    pub struct NodeId;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for LayoutBox {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetainedNode {
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
    pub style: Style,
    pub layout: LayoutBox,
    pub taffy_node: TaffyNodeId,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Div,
    Text { layout: Option<TextLayout> },
}

#[derive(Debug)]
pub struct RetainedTree {
    root: NodeId,
    nodes: SlotMap<NodeId, RetainedNode>,
    taffy: TaffyTree<MeasureContext>,
}

#[derive(Debug, Clone)]
pub struct CompiledScene {
    pub clear_color: Option<crate::style::Color>,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug, Clone)]
pub enum Primitive {
    Quad {
        bounds: LayoutBox,
        color: crate::style::Color,
    },
    Text {
        bounds: LayoutBox,
        layout: TextLayout,
        color: crate::style::Color,
    },
}

#[derive(Debug, Clone)]
enum MeasureContext {
    Text(TextMeasureContext),
}

#[derive(Debug, Clone)]
struct TextMeasureContext {
    text: SharedString,
    style: crate::style::TextStyle,
    last_key: Option<TextMeasureKey>,
    last_layout: Option<TextLayout>,
}

impl TextMeasureContext {
    fn new(text: &Text) -> Self {
        Self {
            text: text.content.clone(),
            style: text.style.text.clone(),
            last_key: None,
            last_layout: None,
        }
    }
}

impl RetainedTree {
    pub fn from_element(root: &Element) -> Self {
        let mut tree = Self {
            root: NodeId::default(),
            nodes: SlotMap::with_key(),
            taffy: TaffyTree::new(),
        };

        let root_id = tree.build_node(root, None);
        tree.root = root_id;
        tree
    }

    pub fn compute_layout(&mut self, size: WindowSize, text_system: &mut TextSystem) {
        let mut measured_layouts = HashMap::<TextMeasureKey, TextLayout>::new();
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
                    } else if let Some(cached_layout) = measured_layouts.get(&key).cloned() {
                        cached_layout
                    } else {
                        let measured =
                            text_system.measure(&text_context.text, &text_context.style, width);
                        measured_layouts.insert(key.clone(), measured.clone());
                        measured
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

        let node_ids = self.nodes.keys().collect::<Vec<_>>();
        for node_id in node_ids {
            let taffy_node = self.nodes[node_id].taffy_node;
            let layout = *self
                .taffy
                .layout(taffy_node)
                .expect("layout must be available after compute_layout");
            self.nodes[node_id].layout = LayoutBox {
                x: layout.location.x,
                y: layout.location.y,
                width: layout.size.width,
                height: layout.size.height,
            };

            if let NodeKind::Text {
                layout: text_layout,
                ..
            } = &mut self.nodes[node_id].kind
            {
                *text_layout =
                    self.taffy
                        .get_node_context(taffy_node)
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

    pub fn compile_scene(&self) -> CompiledScene {
        let mut primitives = Vec::new();
        self.collect_primitives(self.root, 0.0, 0.0, &mut primitives);
        CompiledScene {
            clear_color: None,
            primitives,
        }
    }

    fn build_node(&mut self, element: &Element, parent: Option<NodeId>) -> NodeId {
        match element.kind() {
            ElementKind::Div(div) => self.build_div(div, parent),
            ElementKind::Text(text) => self.build_text(text, parent),
            ElementKind::View(_) => {
                unreachable!("view nodes must be resolved before building the retained tree")
            }
        }
    }

    fn build_div(&mut self, div: &Div, parent: Option<NodeId>) -> NodeId {
        let child_ids = div
            .children
            .iter()
            .map(|child| self.build_node(child, None))
            .collect::<Vec<_>>();
        let child_taffy_nodes = child_ids
            .iter()
            .map(|child_id| self.nodes[*child_id].taffy_node)
            .collect::<Vec<_>>();

        let taffy_node = self
            .taffy
            .new_with_children(div_style_to_taffy(&div.style), &child_taffy_nodes)
            .expect("div node creation must succeed");

        let node_id = self.nodes.insert_with_key(|_id| RetainedNode {
            parent,
            children: child_ids.clone(),
            kind: NodeKind::Div,
            style: div.style.clone(),
            layout: LayoutBox::default(),
            taffy_node,
        });

        for child_id in child_ids {
            self.nodes[child_id].parent = Some(node_id);
        }

        node_id
    }

    fn build_text(&mut self, text: &Text, parent: Option<NodeId>) -> NodeId {
        let taffy_node = self
            .taffy
            .new_leaf_with_context(
                text_style_to_taffy(&text.style),
                MeasureContext::Text(TextMeasureContext::new(text)),
            )
            .expect("text node creation must succeed");

        self.nodes.insert_with_key(|_id| RetainedNode {
            parent,
            children: Vec::new(),
            kind: NodeKind::Text { layout: None },
            style: text.style.clone(),
            layout: LayoutBox::default(),
            taffy_node,
        })
    }

    fn collect_primitives(
        &self,
        node_id: NodeId,
        offset_x: f32,
        offset_y: f32,
        primitives: &mut Vec<Primitive>,
    ) {
        let node = &self.nodes[node_id];
        let bounds = LayoutBox {
            x: offset_x + node.layout.x,
            y: offset_y + node.layout.y,
            width: node.layout.width,
            height: node.layout.height,
        };

        match &node.kind {
            NodeKind::Div => {
                if let Some(background) = node.style.paint.background {
                    primitives.push(Primitive::Quad {
                        bounds,
                        color: background,
                    });
                }
            }
            NodeKind::Text { layout } => {
                if let Some(layout) = layout.clone() {
                    primitives.push(Primitive::Text {
                        bounds,
                        layout,
                        color: node.style.text.color,
                    });
                }
            }
        }

        for child_id in &node.children {
            self.collect_primitives(*child_id, bounds.x, bounds.y, primitives);
        }
    }
}

fn definite_space(space: AvailableSpace) -> Option<f32> {
    match space {
        AvailableSpace::Definite(value) => Some(value),
        AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
    }
}

fn div_style_to_taffy(style: &Style) -> TaffyStyle {
    TaffyStyle {
        display: Display::Flex,
        flex_direction: match style.layout.direction {
            Direction::Row => TaffyFlexDirection::Row,
            Direction::Column => TaffyFlexDirection::Column,
        },
        size: TaffySize {
            width: length_to_dimension(style.layout.size.width),
            height: length_to_dimension(style.layout.size.height),
        },
        margin: Rect {
            left: edge_to_auto(style.layout.margin.left),
            right: edge_to_auto(style.layout.margin.right),
            top: edge_to_auto(style.layout.margin.top),
            bottom: edge_to_auto(style.layout.margin.bottom),
        },
        padding: Rect {
            left: edge_to_length(style.layout.padding.left),
            right: edge_to_length(style.layout.padding.right),
            top: edge_to_length(style.layout.padding.top),
            bottom: edge_to_length(style.layout.padding.bottom),
        },
        gap: TaffySize {
            width: LengthPercentage::length(style.layout.gap),
            height: LengthPercentage::length(style.layout.gap),
        },
        align_items: Some(match style.layout.align_items {
            AlignItems::Start => TaffyAlignItems::Start,
            AlignItems::Center => TaffyAlignItems::Center,
            AlignItems::End => TaffyAlignItems::End,
            AlignItems::Stretch => TaffyAlignItems::Stretch,
        }),
        justify_content: Some(match style.layout.justify_content {
            JustifyContent::Start => TaffyJustifyContent::Start,
            JustifyContent::Center => TaffyJustifyContent::Center,
            JustifyContent::End => TaffyJustifyContent::End,
            JustifyContent::SpaceBetween => TaffyJustifyContent::SpaceBetween,
        }),
        ..Default::default()
    }
}

fn text_style_to_taffy(style: &Style) -> TaffyStyle {
    TaffyStyle {
        display: Display::Block,
        size: TaffySize {
            width: length_to_dimension(style.layout.size.width),
            height: length_to_dimension(style.layout.size.height),
        },
        min_size: TaffySize {
            width: Dimension::length(0.0),
            height: Dimension::AUTO,
        },
        margin: Rect {
            left: edge_to_auto(style.layout.margin.left),
            right: edge_to_auto(style.layout.margin.right),
            top: edge_to_auto(style.layout.margin.top),
            bottom: edge_to_auto(style.layout.margin.bottom),
        },
        ..Default::default()
    }
}

fn length_to_dimension(length: Length) -> Dimension {
    match length {
        Length::Auto => Dimension::AUTO,
        Length::Px(value) => Dimension::length(value),
        Length::Fill => Dimension::percent(1.0),
    }
}

fn edge_to_length(value: f32) -> LengthPercentage {
    LengthPercentage::length(value)
}

fn edge_to_auto(value: f32) -> LengthPercentageAuto {
    LengthPercentageAuto::length(value)
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Render};
    use crate::element::{IntoElement, ParentElement};
    use crate::style::{EdgeInsets, Length};
    use crate::text_system::TextSystem;
    use crate::window::{Window, WindowId, WindowSize};

    use super::{NodeKind, RetainedTree};

    #[test]
    fn text_measurement_wraps_within_available_width() {
        let root = crate::div()
            .width(Length::Px(200.0))
            .padding(EdgeInsets::all(10.0))
            .child(crate::text("hello neko ui hello neko ui hello neko ui").font_size(16.0))
            .into_element();

        let mut tree = RetainedTree::from_element(&root);
        let mut text_system = TextSystem::new();
        tree.compute_layout(WindowSize::new(200, 120), &mut text_system);

        let root_layout = tree.root_layout();
        assert_eq!(root_layout.width, 200.0);

        let text_id = tree.children(tree.root_id())[0];
        let text_node = tree.node(text_id);
        assert!(text_node.layout.width <= 180.0 + 0.5);
        assert!(text_node.layout.height > 20.0);

        match &text_node.kind {
            NodeKind::Text { layout } => {
                let layout = layout.as_ref().expect("text layout exists");
                assert!(layout.runs.len() >= 2);
            }
            NodeKind::Div => panic!("expected text node"),
        }
    }

    #[test]
    fn view_nodes_resolve_before_retained_layout() {
        struct LabelView;

        impl Render for LabelView {
            fn render(
                &mut self,
                _window: &mut Window,
                _cx: &mut crate::Context<'_, Self>,
            ) -> impl IntoElement<Element = crate::Element> {
                crate::text("resolved from view")
            }
        }

        let app = App::new();
        let view = app.insert_view(LabelView);
        let mut window = Window::new_with_metrics(
            WindowId::new(),
            String::from("test"),
            WindowSize::new(320, 200),
            WindowSize::new(640, 400),
            2.0,
        );
        let root = crate::div().child(view).into_element();

        let resolved = app.resolve_root_element(&mut window, &root).unwrap();
        let mut tree = RetainedTree::from_element(&resolved);
        let mut text_system = TextSystem::new();
        tree.compute_layout(window.size(), &mut text_system);

        let child = tree.children(tree.root_id())[0];
        assert!(matches!(tree.node(child).kind, NodeKind::Text { .. }));
    }
}
