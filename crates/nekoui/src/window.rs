use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

impl WindowId {
    pub(crate) fn new() -> Self {
        Self(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl WindowSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone)]
pub struct WindowOptions {
    title: String,
    size: WindowSize,
}

impl WindowOptions {
    pub fn new() -> Self {
        Self {
            title: String::from("NekoUI"),
            size: WindowSize::new(800, 800),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    // TODO: 增加壳层，不操作裸大小
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.size = WindowSize::new(width, height);
        self
    }

    pub fn title_str(&self) -> &str {
        &self.title
    }

    pub fn size_value(&self) -> WindowSize {
        self.size
    }
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle {
    id: WindowId,
}

impl WindowHandle {
    pub(crate) fn new(id: WindowId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> WindowId {
        self.id
    }
}

#[derive(Debug, Clone)]
pub struct Window {
    id: WindowId,
    title: String,
    size: WindowSize,
    physical_size: WindowSize,
    scale_factor: f64,
}

impl Window {
    pub(crate) fn new_with_metrics(
        id: WindowId,
        title: String,
        size: WindowSize,
        physical_size: WindowSize,
        scale_factor: f64,
    ) -> Self {
        Self {
            id,
            title,
            size,
            physical_size,
            scale_factor: sanitize_scale_factor(scale_factor),
        }
    }

    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn size(&self) -> WindowSize {
        self.size
    }

    pub fn physical_size(&self) -> WindowSize {
        self.physical_size
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub(crate) fn set_metrics(
        &mut self,
        size: WindowSize,
        physical_size: WindowSize,
        scale_factor: f64,
    ) {
        self.size = size;
        self.physical_size = physical_size;
        self.scale_factor = sanitize_scale_factor(scale_factor);
    }
}

fn sanitize_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}
