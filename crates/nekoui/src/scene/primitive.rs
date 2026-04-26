use std::ops::Range;
use std::sync::Arc;

use crate::style::{Color, CornerRadii, EdgeWidths, LinearGradient, PaintStyle};
use crate::text_system::SharedTextLayout;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayoutBox {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SceneNodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct PrimitiveRange {
    pub start: u32,
    pub end: u32,
}

#[expect(
    dead_code,
    reason = "Phase 1 scene metadata API is ahead of full renderer usage"
)]
impl PrimitiveRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub fn len(self) -> usize {
        (self.end - self.start) as usize
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub fn as_range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Transform2D {
    pub tx: f32,
    pub ty: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct ClipInfo {
    pub shape: Option<ClipShape>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ClipShape {
    Rect(LayoutBox),
    RoundedRect {
        bounds: LayoutBox,
        corner_radii: CornerRadii,
    },
}

impl ClipShape {
    pub(crate) fn bounds(self) -> LayoutBox {
        match self {
            Self::Rect(bounds) => bounds,
            Self::RoundedRect { bounds, .. } => bounds,
        }
    }

    pub(crate) fn class(self) -> ClipClass {
        match self {
            Self::Rect(_) => ClipClass::Rect,
            Self::RoundedRect { .. } => ClipClass::RoundedRect,
        }
    }

    pub(crate) fn translate(self, dx: f32, dy: f32) -> Self {
        let translate_bounds = |bounds: LayoutBox| LayoutBox {
            x: bounds.x + dx,
            y: bounds.y + dy,
            width: bounds.width,
            height: bounds.height,
        };

        match self {
            Self::Rect(bounds) => Self::Rect(translate_bounds(bounds)),
            Self::RoundedRect {
                bounds,
                corner_radii,
            } => Self::RoundedRect {
                bounds: translate_bounds(bounds),
                corner_radii,
            },
        }
    }
}

pub(crate) type EffectMask = u32;

#[derive(Debug, Clone)]
pub(crate) struct SceneNode {
    pub parent: Option<SceneNodeId>,
    pub first_child: Option<SceneNodeId>,
    pub next_sibling: Option<SceneNodeId>,
    pub transform: Transform2D,
    pub clip: ClipInfo,
    pub opacity: f32,
    pub effect_mask: EffectMask,
    pub primitive_range: PrimitiveRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MaterialClass {
    Rect,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum ClipClass {
    #[default]
    None,
    Rect,
    RoundedRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum EffectClass {
    #[default]
    None,
    Opacity,
}

#[derive(Debug, Clone)]
pub(crate) struct LogicalBatch {
    pub primitive_range: PrimitiveRange,
    pub material_class: MaterialClass,
    pub clip_class: ClipClass,
    pub effect_class: EffectClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum EffectRegionKind {
    #[default]
    None,
}

#[expect(
    dead_code,
    reason = "Effect region metadata is reserved for later scene/render integration"
)]
#[derive(Debug, Clone)]
pub(crate) struct EffectRegion {
    pub bounds: LayoutBox,
    pub kind: EffectRegionKind,
}

#[expect(
    dead_code,
    reason = "Compiled scene carries forward-looking metadata for upcoming passes"
)]
#[derive(Debug, Clone)]
pub(crate) struct CompiledScene {
    pub clear_color: Option<Color>,
    pub scene_nodes: Arc<[SceneNode]>,
    pub primitives: Arc<[Primitive]>,
    pub logical_batches: Arc<[LogicalBatch]>,
    pub effect_regions: Arc<[EffectRegion]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RectFill {
    Solid(Color),
    LinearGradient(LinearGradient),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RectPrimitive {
    pub bounds: LayoutBox,
    pub fill: RectFill,
    pub corner_radii: CornerRadii,
    pub border_widths: EdgeWidths,
    pub border_color: Option<Color>,
    pub opacity: f32,
}

impl RectPrimitive {
    pub(crate) fn from_paint(bounds: LayoutBox, paint: &PaintStyle) -> Option<Self> {
        let background = paint.rect_background()?;
        Some(Self {
            bounds,
            fill: match background {
                crate::style::Background::Solid(color) => RectFill::Solid(color),
                crate::style::Background::LinearGradient(gradient) => {
                    RectFill::LinearGradient(gradient)
                }
            },
            corner_radii: paint.corner_radii,
            border_widths: paint.border.widths,
            border_color: paint.border.color,
            opacity: 1.0,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Primitive {
    Rect(RectPrimitive),
    Text {
        bounds: LayoutBox,
        layout: SharedTextLayout,
        color: Color,
    },
}

impl Primitive {
    pub fn material_class(&self) -> MaterialClass {
        match self {
            Primitive::Rect(_) => MaterialClass::Rect,
            Primitive::Text { .. } => MaterialClass::Text,
        }
    }
}
