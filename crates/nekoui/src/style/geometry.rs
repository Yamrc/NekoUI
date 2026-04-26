#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Px(pub f32);

impl Px {
    pub const fn get(self) -> f32 {
        self.0
    }
}

impl From<f32> for Px {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<Px> for f32 {
    fn from(value: Px) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Rem(pub f32);

impl Rem {
    pub const fn get(self) -> f32 {
        self.0
    }

    pub fn to_px(self, rem_size: Px) -> Px {
        Px(self.0 * rem_size.0)
    }
}

impl From<f32> for Rem {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<Rem> for f32 {
    fn from(value: Rem) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Percent(pub f32);

impl Percent {
    pub const fn get(self) -> f32 {
        self.0
    }
}

impl From<f32> for Percent {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<Percent> for f32 {
    fn from(value: Percent) -> Self {
        value.0
    }
}

pub const fn px(value: f32) -> Px {
    Px(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Point<T = Px> {
    pub x: T,
    pub y: T,
}

impl<T> Point<T> {
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Size<T = Px> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Bounds<T = Px> {
    pub origin: Point<T>,
    pub size: Size<T>,
}

impl<T> Bounds<T> {
    pub const fn new(origin: Point<T>, size: Size<T>) -> Self {
        Self { origin, size }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Absolute {
    Px(Px),
    Rem(Rem),
}

impl Default for Absolute {
    fn default() -> Self {
        Self::Px(Px::default())
    }
}

impl Absolute {
    pub fn to_px(self, rem_size: Px) -> Px {
        match self {
            Self::Px(px) => px,
            Self::Rem(rem) => rem.to_px(rem_size),
        }
    }
}

impl From<Px> for Absolute {
    fn from(value: Px) -> Self {
        Self::Px(value)
    }
}

impl From<Rem> for Absolute {
    fn from(value: Rem) -> Self {
        Self::Rem(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Definite {
    Absolute(Absolute),
    Percent(Percent),
}

impl Default for Definite {
    fn default() -> Self {
        Self::Absolute(Absolute::default())
    }
}

impl Definite {
    pub fn to_px(self, base: Px, rem_size: Px) -> Px {
        match self {
            Self::Absolute(absolute) => absolute.to_px(rem_size),
            Self::Percent(percent) => Px(base.0 * percent.0),
        }
    }
}

impl From<Px> for Definite {
    fn from(value: Px) -> Self {
        Self::Absolute(value.into())
    }
}

impl From<Rem> for Definite {
    fn from(value: Rem) -> Self {
        Self::Absolute(value.into())
    }
}

impl From<Absolute> for Definite {
    fn from(value: Absolute) -> Self {
        Self::Absolute(value)
    }
}

impl From<Percent> for Definite {
    fn from(value: Percent) -> Self {
        Self::Percent(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    #[default]
    Auto,
    Definite(Definite),
    Fill,
}

impl From<Px> for Length {
    fn from(value: Px) -> Self {
        Self::Definite(value.into())
    }
}

impl From<Rem> for Length {
    fn from(value: Rem) -> Self {
        Self::Definite(value.into())
    }
}

impl From<Percent> for Length {
    fn from(value: Percent) -> Self {
        Self::Definite(value.into())
    }
}

impl From<Absolute> for Length {
    fn from(value: Absolute) -> Self {
        Self::Definite(value.into())
    }
}

impl From<Definite> for Length {
    fn from(value: Definite) -> Self {
        Self::Definite(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Edges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T> Edges<T> {
    pub const fn new(top: T, right: T, bottom: T, left: T) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

impl<T> Edges<T>
where
    T: Clone,
{
    pub fn all(value: T) -> Self {
        Self {
            top: value.clone(),
            right: value.clone(),
            bottom: value.clone(),
            left: value,
        }
    }

    pub fn xy(x: T, y: T) -> Self {
        Self {
            top: y.clone(),
            right: x.clone(),
            bottom: y,
            left: x,
        }
    }
}

impl<T> Edges<T> {
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Edges<U> {
        Edges {
            top: f(self.top),
            right: f(self.right),
            bottom: f(self.bottom),
            left: f(self.left),
        }
    }
}

impl<T> Edges<T>
where
    T: Clone + Default,
{
    pub fn x(value: T) -> Self {
        Self {
            top: T::default(),
            right: value.clone(),
            bottom: T::default(),
            left: value,
        }
    }

    pub fn y(value: T) -> Self {
        Self {
            top: value.clone(),
            right: T::default(),
            bottom: value,
            left: T::default(),
        }
    }

    pub fn horizontal(value: T) -> Self {
        Self::x(value)
    }

    pub fn vertical(value: T) -> Self {
        Self::y(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Corners<T> {
    pub top_left: T,
    pub top_right: T,
    pub bottom_right: T,
    pub bottom_left: T,
}

impl<T> Corners<T> {
    pub const fn new(top_left: T, top_right: T, bottom_right: T, bottom_left: T) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }
}

impl<T> Corners<T>
where
    T: Clone,
{
    pub fn all(value: T) -> Self {
        Self {
            top_left: value.clone(),
            top_right: value.clone(),
            bottom_right: value.clone(),
            bottom_left: value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Gap<T> {
    pub row: T,
    pub column: T,
}

impl<T> Gap<T> {
    pub const fn new(row: T, column: T) -> Self {
        Self { row, column }
    }
}

impl<T> Gap<T>
where
    T: Clone,
{
    pub fn all(value: T) -> Self {
        Self {
            row: value.clone(),
            column: value,
        }
    }
}

impl<T> Gap<T> {
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Gap<U> {
        Gap {
            row: f(self.row),
            column: f(self.column),
        }
    }
}

impl Size<Length> {
    pub const fn fill() -> Self {
        Self {
            width: Length::Fill,
            height: Length::Fill,
        }
    }
}

pub type LayoutSize = Size<Length>;
pub type EdgeInsets = Edges<f32>;
pub type EdgeWidths = Edges<f32>;
pub type CornerRadii = Corners<f32>;

impl From<Edges<f32>> for Edges<Definite> {
    fn from(value: Edges<f32>) -> Self {
        value.map(|component| Definite::from(Px(component)))
    }
}

impl From<Edges<f32>> for Edges<Length> {
    fn from(value: Edges<f32>) -> Self {
        value.map(|component| Length::from(Px(component)))
    }
}

impl From<Gap<f32>> for Gap<Definite> {
    fn from(value: Gap<f32>) -> Self {
        value.map(|component| Definite::from(Px(component)))
    }
}

impl From<f32> for Gap<Definite> {
    fn from(value: f32) -> Self {
        Gap::all(Definite::from(Px(value)))
    }
}

pub const fn rem(value: f32) -> Rem {
    Rem(value)
}

pub const fn percent(value: f32) -> Percent {
    Percent(value)
}

pub const fn point<T>(x: T, y: T) -> Point<T> {
    Point::new(x, y)
}

pub const fn size<T>(width: T, height: T) -> Size<T> {
    Size::new(width, height)
}

pub const fn bounds<T>(origin: Point<T>, size: Size<T>) -> Bounds<T> {
    Bounds::new(origin, size)
}

#[cfg(test)]
mod tests {
    use super::{
        Absolute, Bounds, CornerRadii, Corners, Definite, EdgeInsets, EdgeWidths, Edges, Gap,
        LayoutSize, Length, Percent, Point, Px, Rem, Size, bounds, percent, point, px, rem, size,
    };

    #[test]
    fn geometry_aliases_preserve_values() {
        let origin = point(px(12.0), px(24.0));
        let extent = size(px(320.0), px(240.0));
        let rect = bounds(origin, extent);

        assert_eq!(
            origin,
            Point {
                x: Px(12.0),
                y: Px(24.0),
            }
        );
        assert_eq!(
            extent,
            Size {
                width: Px(320.0),
                height: Px(240.0),
            }
        );
        assert_eq!(
            rect,
            Bounds {
                origin,
                size: extent,
            }
        );
    }

    #[test]
    fn new_units_convert_into_length_family() {
        assert_eq!(
            Length::from(Px(10.0)),
            Length::Definite(Definite::from(Px(10.0)))
        );
        assert_eq!(
            Length::from(rem(2.0)),
            Length::Definite(Definite::from(Rem(2.0)))
        );
        assert_eq!(
            Length::from(percent(0.5)),
            Length::Definite(Definite::from(Percent(0.5)))
        );
        assert_eq!(Absolute::from(Rem(1.5)).to_px(Px(16.0)), Px(24.0));
    }

    #[test]
    fn generic_shorthand_helpers_replace_duplicate_wrappers() {
        assert_eq!(
            EdgeInsets::all(8.0),
            Edges {
                top: 8.0,
                right: 8.0,
                bottom: 8.0,
                left: 8.0,
            }
        );
        assert_eq!(
            EdgeWidths::xy(2.0, 4.0),
            Edges {
                top: 4.0,
                right: 2.0,
                bottom: 4.0,
                left: 2.0,
            }
        );
        assert_eq!(CornerRadii::all(6.0), Corners::all(6.0));
        assert_eq!(
            Gap::all(12.0),
            Gap {
                row: 12.0,
                column: 12.0
            }
        );
        assert_eq!(LayoutSize::fill(), size(Length::Fill, Length::Fill));
    }
}
