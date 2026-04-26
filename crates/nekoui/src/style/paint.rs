use super::color::Color;
use super::geometry::{CornerRadii, EdgeWidths};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearGradient {
    pub start_color: Color,
    pub end_color: Color,
    pub angle_radians: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Background {
    Solid(Color),
    LinearGradient(LinearGradient),
}

pub type BackgroundFill = Background;

impl From<Color> for Background {
    fn from(value: Color) -> Self {
        Self::Solid(value)
    }
}

pub fn gradient(start_color: Color, end_color: Color, angle_radians: f32) -> Background {
    Background::LinearGradient(LinearGradient {
        start_color,
        end_color,
        angle_radians,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub widths: EdgeWidths,
    pub color: Option<Color>,
}

impl Border {
    pub const fn none() -> Self {
        Self {
            widths: EdgeWidths {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            color: None,
        }
    }

    pub fn all(width: f32, color: Color) -> Self {
        Self {
            widths: EdgeWidths::all(width.max(0.0)),
            color: Some(color),
        }
    }

    pub fn has_visible_edge(self) -> bool {
        self.color.is_some()
            && (self.widths.top > 0.0
                || self.widths.right > 0.0
                || self.widths.bottom > 0.0
                || self.widths.left > 0.0)
    }
}

impl Default for Border {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaintStyle {
    pub background: Option<Background>,
    pub corner_radii: CornerRadii,
    pub border: Border,
    pub opacity: f32,
}

impl Default for PaintStyle {
    fn default() -> Self {
        Self {
            background: None,
            corner_radii: CornerRadii::default(),
            border: Border::default(),
            opacity: 1.0,
        }
    }
}

impl PaintStyle {
    pub fn has_visible_border(&self) -> bool {
        self.border.has_visible_edge()
    }

    pub fn rect_background(&self) -> Option<Background> {
        self.background.or_else(|| {
            self.has_visible_border()
                .then_some(Background::Solid(Color::rgba(0.0, 0.0, 0.0, 0.0)))
        })
    }
}
