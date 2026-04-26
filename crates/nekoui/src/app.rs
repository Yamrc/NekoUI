mod context;
mod executor;
mod handle;
mod runtime;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::element::AnyElement;
use crate::error::Error;
use crate::platform;
use crate::window::WindowInfo;

pub use context::{AppContext, Context, EventEmitter, Subscription};
pub use executor::{BackgroundExecutor, Task, TaskResult, UiExecutor};
pub use handle::{Entity, View, WeakEntity};
pub(crate) use runtime::{App, PendingWindowRequest};

pub(crate) type WakeHandle = Arc<dyn Fn() + Send + Sync>;
pub(crate) type WindowBuildFn =
    Box<dyn FnOnce(&WindowInfo, &mut runtime::App) -> AnyElement + 'static>;

pub struct Application {
    last_window_behavior: LastWindowBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LastWindowBehavior {
    ExitEventLoop,
    KeepEventLoopAlive,
}

impl Application {
    pub fn new() -> Self {
        Self {
            last_window_behavior: LastWindowBehavior::ExitEventLoop,
        }
    }

    pub fn last_window_behavior(mut self, behavior: LastWindowBehavior) -> Self {
        self.last_window_behavior = behavior;
        self
    }

    pub fn run(
        self,
        on_launch: impl FnOnce(&mut AppContext<'_>) -> Result<(), Error> + 'static,
    ) -> Result<(), Error> {
        platform::run_application(self.last_window_behavior, on_launch)
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Render: 'static + Sized {
    fn render(
        &mut self,
        window: &WindowInfo,
        cx: &mut Context<'_, Self>,
    ) -> impl crate::IntoElement;
}
