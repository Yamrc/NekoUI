use crate::SharedString;
use crate::style::{AlignItems, Color, Direction, EdgeInsets, JustifyContent, Length, Size, Style};

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    kind: ElementKind,
}

impl Element {
    pub(crate) fn new(kind: ElementKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> &ElementKind {
        &self.kind
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElementKind {
    Div(Div),
    Text(Text),
    View(ViewSpec),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewSpec {
    pub entity_id: u64,
}

pub trait IntoElement {
    type Element;

    fn into_element(self) -> Self::Element;
}

pub trait ParentElement {
    fn child(self, child: impl IntoElement<Element = Element>) -> Self
    where
        Self: Sized;

    fn children(
        self,
        children: impl IntoIterator<Item = impl IntoElement<Element = Element>>,
    ) -> Self
    where
        Self: Sized;
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Div {
    pub(crate) key: Option<u64>,
    pub(crate) style: Style,
    pub(crate) children: Vec<Element>,
}

pub fn div() -> Div {
    Div::default()
}

impl Div {
    pub fn key(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.style.layout.size = size;
        self
    }

    pub fn width(mut self, width: Length) -> Self {
        self.style.layout.size.width = width;
        self
    }

    pub fn height(mut self, height: Length) -> Self {
        self.style.layout.size.height = height;
        self
    }

    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.style.layout.padding = padding;
        self
    }

    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.style.layout.margin = margin;
        self
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.style.layout.direction = direction;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.style.layout.gap = gap;
        self
    }

    pub fn justify(mut self, justify_content: JustifyContent) -> Self {
        self.style.layout.justify_content = justify_content;
        self
    }

    pub fn align_items(mut self, align_items: AlignItems) -> Self {
        self.style.layout.align_items = align_items;
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.style.paint.background = Some(color);
        self
    }
}

impl ParentElement for Div {
    fn child(mut self, child: impl IntoElement<Element = Element>) -> Self {
        self.children.push(child.into_element());
        self
    }

    fn children(
        mut self,
        children: impl IntoIterator<Item = impl IntoElement<Element = Element>>,
    ) -> Self {
        self.children
            .extend(children.into_iter().map(IntoElement::into_element));
        self
    }
}

impl IntoElement for Div {
    type Element = Element;

    fn into_element(self) -> Self::Element {
        Element::new(ElementKind::Div(self))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub(crate) key: Option<u64>,
    pub(crate) style: Style,
    pub(crate) content: SharedString,
}

pub fn text(content: impl Into<SharedString>) -> Text {
    Text {
        key: None,
        style: Style::default(),
        content: content.into(),
    }
}

impl Text {
    pub fn key(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.style.text.font_size = font_size.max(1.0);
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.style.text.line_height = Some(line_height.max(1.0));
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.style.text.font_family = Some(family.into());
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.style.text.color = color;
        self
    }
}

impl IntoElement for Text {
    type Element = Element;

    fn into_element(self) -> Self::Element {
        Element::new(ElementKind::Text(self))
    }
}

impl IntoElement for Element {
    type Element = Element;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl IntoElement for () {
    type Element = Element;

    fn into_element(self) -> Self::Element {
        div().into_element()
    }
}
