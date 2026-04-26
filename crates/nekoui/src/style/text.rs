use std::sync::Arc;

use crate::SharedString;

use super::color::Color;
use super::geometry::{Absolute, Definite, Px};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FontFamily {
    Named(SharedString),
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    SystemUi,
}

impl From<SharedString> for FontFamily {
    fn from(value: SharedString) -> Self {
        Self::Named(value)
    }
}

impl From<String> for FontFamily {
    fn from(value: String) -> Self {
        Self::Named(SharedString::from(value))
    }
}

impl From<&str> for FontFamily {
    fn from(value: &str) -> Self {
        Self::Named(SharedString::from(value))
    }
}

pub trait IntoFontFamilies {
    fn into_font_families(self) -> Arc<[FontFamily]>;
}

impl<T> IntoFontFamilies for T
where
    T: Into<FontFamily>,
{
    fn into_font_families(self) -> Arc<[FontFamily]> {
        Arc::from(vec![self.into()])
    }
}

impl<T, const N: usize> IntoFontFamilies for [T; N]
where
    T: Into<FontFamily>,
{
    fn into_font_families(self) -> Arc<[FontFamily]> {
        Arc::from(
            self.into_iter()
                .map(Into::into)
                .collect::<Vec<FontFamily>>()
                .into_boxed_slice(),
        )
    }
}

impl<T> IntoFontFamilies for Vec<T>
where
    T: Into<FontFamily>,
{
    fn into_font_families(self) -> Arc<[FontFamily]> {
        Arc::from(
            self.into_iter()
                .map(Into::into)
                .collect::<Vec<FontFamily>>()
                .into_boxed_slice(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontWeight {
    #[default]
    Normal,
    Medium,
    Semibold,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    Nowrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyle {
    pub font_families: Option<Arc<[FontFamily]>>,
    pub font_size: Option<Absolute>,
    pub line_height: Option<Definite>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub text_align: Option<TextAlign>,
    pub white_space: Option<WhiteSpace>,
    pub text_overflow: Option<TextOverflow>,
    pub color: Option<Color>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTextStyle {
    pub font_families: Arc<[FontFamily]>,
    pub font_size: Absolute,
    pub line_height: Option<Definite>,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub text_align: TextAlign,
    pub white_space: WhiteSpace,
    pub text_overflow: Option<TextOverflow>,
    pub color: Color,
}

impl Default for ResolvedTextStyle {
    fn default() -> Self {
        Self {
            font_families: Arc::from([FontFamily::SansSerif]),
            font_size: Absolute::from(Px(14.0)),
            line_height: None,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            text_align: TextAlign::Start,
            white_space: WhiteSpace::Normal,
            text_overflow: None,
            color: Color::default(),
        }
    }
}

impl TextStyle {
    pub fn resolve_with_parent(&self, parent: &ResolvedTextStyle) -> ResolvedTextStyle {
        ResolvedTextStyle {
            font_families: self
                .font_families
                .clone()
                .unwrap_or_else(|| parent.font_families.clone()),
            font_size: self.font_size.unwrap_or(parent.font_size),
            line_height: self.line_height.or(parent.line_height),
            font_weight: self.font_weight.unwrap_or(parent.font_weight),
            font_style: self.font_style.unwrap_or(parent.font_style),
            text_align: self.text_align.unwrap_or(parent.text_align),
            white_space: self.white_space.unwrap_or(parent.white_space),
            text_overflow: self.text_overflow,
            color: self.color.unwrap_or(parent.color),
        }
    }

    pub fn resolves_to_same_inherited_fields(&self, other: &Self) -> bool {
        self.font_families == other.font_families
            && self.font_size == other.font_size
            && self.line_height == other.line_height
            && self.font_weight == other.font_weight
            && self.font_style == other.font_style
            && self.text_align == other.text_align
            && self.white_space == other.white_space
            && self.color == other.color
    }
}
