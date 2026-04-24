use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::SharedString;
use crate::error::RuntimeError;

use super::command::{WindowCommand, WindowCommandSender};
use super::model::{DisplaySelector, WindowGeometryPatch};

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

impl WindowId {
    pub(crate) fn new() -> Self {
        Self(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) const fn raw(self) -> u64 {
        self.0
    }
}

pub struct WindowHandle<V> {
    id: WindowId,
    commands: WindowCommandSender,
    marker: PhantomData<fn() -> V>,
}

impl<V> Clone for WindowHandle<V> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            commands: self.commands.clone(),
            marker: PhantomData,
        }
    }
}

impl<V> PartialEq for WindowHandle<V> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<V> Eq for WindowHandle<V> {}

impl<V> Hash for WindowHandle<V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<V> WindowHandle<V> {
    pub(crate) fn new(id: WindowId, commands: WindowCommandSender) -> Self {
        Self {
            id,
            commands,
            marker: PhantomData,
        }
    }

    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn close(&self) -> Result<(), RuntimeError> {
        self.commands.send(self.id, WindowCommand::Close)
    }

    pub fn focus(&self) -> Result<(), RuntimeError> {
        self.commands.send(self.id, WindowCommand::Focus)
    }

    pub fn request_redraw(&self) -> Result<(), RuntimeError> {
        self.commands.send(self.id, WindowCommand::RequestRedraw)
    }

    pub fn set_title(&self, title: impl Into<SharedString>) -> Result<(), RuntimeError> {
        self.commands
            .send(self.id, WindowCommand::SetTitle(title.into()))
    }

    pub fn set_geometry(&self, patch: WindowGeometryPatch) -> Result<(), RuntimeError> {
        self.commands
            .send(self.id, WindowCommand::SetGeometry(patch))
    }

    pub fn set_visible(&self, visible: bool) -> Result<(), RuntimeError> {
        self.commands
            .send(self.id, WindowCommand::SetVisible(visible))
    }

    pub fn set_resizable(&self, resizable: bool) -> Result<(), RuntimeError> {
        self.commands
            .send(self.id, WindowCommand::SetResizable(resizable))
    }

    pub fn maximize(&self) -> Result<(), RuntimeError> {
        self.commands.send(self.id, WindowCommand::Maximize)
    }

    pub fn unmaximize(&self) -> Result<(), RuntimeError> {
        self.commands.send(self.id, WindowCommand::Unmaximize)
    }

    pub fn fullscreen(&self, display: Option<DisplaySelector>) -> Result<(), RuntimeError> {
        self.commands
            .send(self.id, WindowCommand::Fullscreen(display))
    }

    pub fn exit_fullscreen(&self) -> Result<(), RuntimeError> {
        self.commands.send(self.id, WindowCommand::ExitFullscreen)
    }

    pub fn minimize(&self) -> Result<(), RuntimeError> {
        self.commands.send(self.id, WindowCommand::Minimize)
    }
}
