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

pub const fn px(value: f32) -> Px {
    Px(value)
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
    use super::{Bounds, Point, Px, Size, bounds, point, px, size};

    #[test]
    fn constructors_preserve_values() {
        let x = px(12.0);
        let y = px(24.0);
        let origin = point(x, y);
        let extent = size(px(320.0), px(240.0));
        let rect = bounds(origin, extent);

        assert_eq!(x, Px(12.0));
        assert_eq!(origin, Point { x, y });
        assert_eq!(
            extent,
            Size {
                width: px(320.0),
                height: px(240.0),
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
}
