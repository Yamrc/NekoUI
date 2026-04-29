mod color;
pub mod geometry;
mod layout;
mod paint;
mod runtime;
mod text;

pub use color::{Color, Oklch};
pub use geometry::{
    Absolute, Bounds, CornerRadii, Corners, Definite, EdgeInsets, EdgeWidths, Edges, Gap,
    LayoutSize, Length, Percent, Point, Px, Rem, Size, bounds, percent, point, px, rem, size,
};
pub use layout::{
    AlignItems, AlignSelf, BoxSizing, Direction, Display, FlexDirection, FlexWrap, JustifyContent,
    LayoutStyle, Overflow,
};
pub use paint::{Background, BackgroundFill, Border, LinearGradient, PaintStyle, gradient};
pub(crate) use runtime::{
    StyleChange, diff_div_style, diff_text_style, div_style_to_taffy, text_style_to_taffy,
};
pub use text::{
    FontFamily, FontStyle, FontWeight, IntoFontFamilies, ResolvedTextStyle, TextAlign,
    TextOverflow, TextStyle, WhiteSpace,
};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    pub layout: LayoutStyle,
    pub paint: PaintStyle,
    pub text: TextStyle,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvedStyle {
    pub layout: LayoutStyle,
    pub paint: PaintStyle,
    pub text: ResolvedTextStyle,
}

impl Style {
    pub fn resolve_with_parent(&self, parent_text: &ResolvedTextStyle) -> ResolvedStyle {
        ResolvedStyle {
            layout: self.layout.clone(),
            paint: self.paint.clone(),
            text: self.text.resolve_with_parent(parent_text),
        }
    }
}
