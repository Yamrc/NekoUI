use crossbeam_channel::Sender;

use crate::SharedString;
use crate::error::RuntimeError;

use super::handle::WindowId;
use super::model::{DisplaySelector, WindowGeometryPatch};

#[derive(Debug, Clone)]
pub(crate) enum WindowCommand {
    Close,
    Focus,
    RequestRedraw,
    SetTitle(SharedString),
    SetGeometry(WindowGeometryPatch),
    SetVisible(bool),
    SetResizable(bool),
    Maximize,
    Unmaximize,
    Fullscreen(Option<DisplaySelector>),
    ExitFullscreen,
    Minimize,
}

#[derive(Clone)]
pub(crate) struct WindowCommandSender {
    sender: Sender<(WindowId, WindowCommand)>,
}

impl WindowCommandSender {
    pub(crate) fn new(sender: Sender<(WindowId, WindowCommand)>) -> Self {
        Self { sender }
    }

    pub(crate) fn send(
        &self,
        window_id: WindowId,
        command: WindowCommand,
    ) -> Result<(), RuntimeError> {
        self.sender
            .send((window_id, command))
            .map_err(|_| RuntimeError::WindowCommandUnavailable(window_id.raw()))
    }
}
