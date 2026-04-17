#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn rgb(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
        let b = (hex & 0xFF) as f32 / 255.0;
        Self { r, g, b, a: 1.0 }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::rgba(0.0, 0.0, 0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    #[default]
    Auto,
    Px(f32),
    Fill,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: Length,
    pub height: Length,
}

impl Size {
    pub const fn new(width: Length, height: Length) -> Self {
        Self { width, height }
    }
}

impl Default for Size {
    fn default() -> Self {
        Self::new(Length::Auto, Length::Auto)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl EdgeInsets {
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

impl Default for EdgeInsets {
    fn default() -> Self {
        Self::all(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Row,
    Column,
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

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutStyle {
    pub size: Size,
    pub padding: EdgeInsets,
    pub margin: EdgeInsets,
    pub direction: Direction,
    pub gap: f32,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            size: Size::default(),
            padding: EdgeInsets::default(),
            margin: EdgeInsets::default(),
            direction: Direction::default(),
            gap: 0.0,
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaintStyle {
    pub background: Option<Color>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub font_family: Option<String>,
    pub font_size: f32,
    pub line_height: Option<f32>,
    pub color: Color,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 14.0,
            line_height: None,
            color: Color::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    pub layout: LayoutStyle,
    pub paint: PaintStyle,
    pub text: TextStyle,
}
