use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use hashbrown::{HashMap, HashSet};

use crate::element::{Element, ElementKind, IntoElement, ViewSpec};
use crate::error::{Error, RuntimeError};
use crate::platform;
use crate::window::{Window, WindowHandle, WindowId, WindowOptions};

static NEXT_ENTITY_ID: AtomicU64 = AtomicU64::new(1);

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
        on_launch: impl FnOnce(&mut App) -> Result<(), Error> + 'static,
    ) -> Result<(), Error> {
        platform::run_application(self.last_window_behavior, on_launch)
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct RuntimeState {
    entities: HashMap<u64, Box<dyn Any>>,
    dirty_entities: HashSet<u64>,
    view_renderers: HashMap<u64, ViewRenderer>,
}

type WakeHandle = Arc<dyn Fn() + Send + Sync>;
type WindowBuildFn = Box<dyn FnOnce(&mut Window, &mut App) -> Element + 'static>;
type ViewRenderer =
    fn(u64, &Rc<RefCell<RuntimeState>>, &mut Window) -> Result<Element, RuntimeError>;

pub(crate) struct PendingWindowRequest {
    pub id: WindowId,
    pub options: WindowOptions,
    pub build_root: WindowBuildFn,
}

pub struct App {
    runtime: Rc<RefCell<RuntimeState>>,
    pending_windows: VecDeque<PendingWindowRequest>,
    wake_handle: Option<WakeHandle>,
}

impl App {
    pub(crate) fn new() -> Self {
        Self {
            runtime: Rc::new(RefCell::new(RuntimeState::default())),
            pending_windows: VecDeque::new(),
            wake_handle: None,
        }
    }

    pub(crate) fn set_wake_handle(&mut self, wake_handle: Option<WakeHandle>) {
        self.wake_handle = wake_handle;
    }

    fn wake_runtime(&self) {
        if let Some(wake_handle) = &self.wake_handle {
            wake_handle();
        }
    }

    pub fn open_window<E>(
        &mut self,
        options: WindowOptions,
        build_root: impl FnOnce(&mut Window, &mut App) -> E + 'static,
    ) -> Result<WindowHandle, Error>
    where
        E: IntoElement<Element = Element>,
    {
        let id = WindowId::new();
        self.pending_windows.push_back(PendingWindowRequest {
            id,
            options,
            build_root: Box::new(move |window, app| build_root(window, app).into_element()),
        });
        self.wake_runtime();
        Ok(WindowHandle::new(id))
    }

    pub fn insert_entity<T>(&self, state: T) -> Entity<T>
    where
        T: 'static,
    {
        let entity = Entity::new();
        self.runtime
            .borrow_mut()
            .entities
            .insert(entity.id, Box::new(state));
        entity
    }

    pub fn insert_view<T>(&self, state: T) -> View<T>
    where
        T: Render + 'static,
    {
        let view = View::new();
        let mut runtime = self.runtime.borrow_mut();
        runtime.entities.insert(view.id, Box::new(state));
        runtime
            .view_renderers
            .insert(view.id, render_view_entity::<T>);
        view
    }

    pub fn update<T, R>(
        &self,
        entity: Entity<T>,
        updater: impl FnOnce(&mut T, &mut Context<'_, T>) -> R,
    ) -> Result<R, RuntimeError>
    where
        T: 'static,
    {
        let boxed = self
            .runtime
            .borrow_mut()
            .entities
            .remove(&entity.id)
            .ok_or(RuntimeError::EntityNotFound(entity.id))?;

        let mut typed = boxed
            .downcast::<T>()
            .map_err(|_| RuntimeError::TypeMismatch(entity.id))?;

        let result = {
            let mut cx = Context::new(entity, self.runtime.clone());
            updater(typed.as_mut(), &mut cx)
        };

        self.runtime
            .borrow_mut()
            .entities
            .insert(entity.id, typed as Box<dyn Any>);

        Ok(result)
    }

    pub(crate) fn drain_window_requests(&mut self) -> Vec<PendingWindowRequest> {
        self.pending_windows.drain(..).collect()
    }

    pub(crate) fn take_dirty_entities(&self) -> Vec<u64> {
        self.runtime.borrow_mut().dirty_entities.drain().collect()
    }

    pub(crate) fn resolve_root_element(
        &self,
        window: &mut Window,
        template: &Element,
    ) -> Result<Element, RuntimeError> {
        self.resolve_element(window, template)
    }

    pub(crate) fn make_runtime_window(
        id: WindowId,
        options: &WindowOptions,
        size: crate::window::WindowSize,
        physical_size: crate::window::WindowSize,
        scale_factor: f64,
    ) -> Window {
        Window::new_with_metrics(
            id,
            options.title_str().to_string(),
            size,
            physical_size,
            scale_factor,
        )
    }

    fn resolve_element(
        &self,
        window: &mut Window,
        template: &Element,
    ) -> Result<Element, RuntimeError> {
        match template.kind() {
            ElementKind::Div(div) => {
                let mut resolved = div.clone();
                resolved.children = div
                    .children
                    .iter()
                    .map(|child| self.resolve_element(window, child))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(resolved.into_element())
            }
            ElementKind::Text(text) => Ok(text.clone().into_element()),
            ElementKind::View(ViewSpec { entity_id }) => {
                let renderer = self
                    .runtime
                    .borrow()
                    .view_renderers
                    .get(entity_id)
                    .copied()
                    .ok_or(RuntimeError::EntityNotFound(*entity_id))?;
                let rendered = renderer(*entity_id, &self.runtime, window)?;
                self.resolve_element(window, &rendered)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Entity<T> {
    id: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for Entity<T> {}

impl<T> Clone for Entity<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Entity<T> {
    fn new() -> Self {
        Self {
            id: NEXT_ENTITY_ID.fetch_add(1, Ordering::Relaxed),
            marker: PhantomData,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn downgrade(self) -> WeakEntity<T> {
        WeakEntity {
            id: self.id,
            marker: PhantomData,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct WeakEntity<T> {
    id: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for WeakEntity<T> {}

impl<T> Clone for WeakEntity<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> WeakEntity<T> {
    pub fn id(&self) -> u64 {
        self.id
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct View<T> {
    id: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for View<T> {}

impl<T> Clone for View<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> View<T> {
    fn new() -> Self {
        Self {
            id: NEXT_ENTITY_ID.fetch_add(1, Ordering::Relaxed),
            marker: PhantomData,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn entity(self) -> Entity<T> {
        Entity {
            id: self.id,
            marker: PhantomData,
        }
    }
}

impl<T> IntoElement for View<T> {
    type Element = Element;

    fn into_element(self) -> Self::Element {
        Element::new(ElementKind::View(ViewSpec { entity_id: self.id }))
    }
}

pub struct Context<'a, T> {
    entity: Entity<T>,
    runtime: Rc<RefCell<RuntimeState>>,
    marker: PhantomData<&'a mut T>,
}

impl<'a, T> Context<'a, T> {
    fn new(entity: Entity<T>, runtime: Rc<RefCell<RuntimeState>>) -> Self {
        Self {
            entity,
            runtime,
            marker: PhantomData,
        }
    }

    pub fn entity(&self) -> Entity<T> {
        self.entity
    }

    pub fn notify(&mut self) {
        self.runtime
            .borrow_mut()
            .dirty_entities
            .insert(self.entity.id);
    }
}

pub trait Render: 'static + Sized {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> impl IntoElement<Element = Element>;
}

fn render_view_entity<T>(
    entity_id: u64,
    runtime: &Rc<RefCell<RuntimeState>>,
    window: &mut Window,
) -> Result<Element, RuntimeError>
where
    T: Render + 'static,
{
    let boxed = runtime
        .borrow_mut()
        .entities
        .remove(&entity_id)
        .ok_or(RuntimeError::EntityNotFound(entity_id))?;

    let mut typed = boxed
        .downcast::<T>()
        .map_err(|_| RuntimeError::TypeMismatch(entity_id))?;

    let rendered = {
        let mut cx = Context::new(
            Entity {
                id: entity_id,
                marker: PhantomData,
            },
            runtime.clone(),
        );
        typed.render(window, &mut cx).into_element()
    };

    runtime
        .borrow_mut()
        .entities
        .insert(entity_id, typed as Box<dyn Any>);

    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::{App, Application, LastWindowBehavior};
    use crate::window::WindowOptions;

    #[test]
    fn application_defaults_to_exit_when_last_window_closes() {
        let application = Application::new();
        assert_eq!(
            application.last_window_behavior,
            LastWindowBehavior::ExitEventLoop
        );
    }

    #[test]
    fn open_window_queues_request() {
        let mut app = App::new();
        let handle = app
            .open_window(WindowOptions::new().title("Neko"), |_window, _app| ())
            .unwrap();

        let requests = app.drain_window_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id, handle.id());
    }

    #[test]
    fn update_round_trips_entity_state() {
        let app = App::new();
        let entity = app.insert_entity(String::from("neko"));

        let updated = app
            .update(entity, |value, cx| {
                value.push_str(" ui");
                cx.notify();
                value.clone()
            })
            .unwrap();

        assert_eq!(updated, "neko ui");
    }
}
