use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crossbeam_channel::{Receiver, Sender, bounded};
use parking_lot::Mutex;

use super::WakeHandle;

pub enum TaskResult<T> {
    Ready(T),
    Canceled,
}

pub struct Task<T> {
    receiver: Receiver<TaskResult<T>>,
    canceled: Arc<AtomicBool>,
}

impl<T> Task<T> {
    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Relaxed);
    }

    pub fn try_recv(&self) -> Option<TaskResult<T>> {
        self.receiver.try_recv().ok()
    }

    pub fn recv(self) -> TaskResult<T> {
        self.receiver.recv().unwrap_or(TaskResult::Canceled)
    }
}

impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Clone)]
pub struct BackgroundExecutor {
    sender: Sender<Box<dyn FnOnce() + Send + 'static>>,
}

impl BackgroundExecutor {
    pub(super) fn new() -> Self {
        let worker_count = thread::available_parallelism()
            .map(|value| value.get().min(4))
            .unwrap_or(2)
            .max(1);
        let (sender, receiver) = bounded::<Box<dyn FnOnce() + Send + 'static>>(256);

        for index in 0..worker_count {
            let worker_receiver = receiver.clone();
            thread::Builder::new()
                .name(format!("nekoui-bg-{index}"))
                .spawn(move || {
                    while let Ok(job) = worker_receiver.recv() {
                        job();
                    }
                })
                .expect("background executor worker must start");
        }

        Self { sender }
    }

    pub fn spawn<F>(&self, future: F) -> Task<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.spawn_job(move || pollster::block_on(future))
    }

    pub fn spawn_blocking<F, R>(&self, f: F) -> Task<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.spawn_job(f)
    }

    fn spawn_job<F, R>(&self, job: F) -> Task<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (result_sender, result_receiver) = bounded(1);
        let canceled = Arc::new(AtomicBool::new(false));
        let canceled_for_job = canceled.clone();

        let enqueue_result = self.sender.try_send(Box::new(move || {
            if canceled_for_job.load(Ordering::Relaxed) {
                let _ = result_sender.send(TaskResult::Canceled);
                return;
            }

            let output = job();
            let result = if canceled_for_job.load(Ordering::Relaxed) {
                TaskResult::Canceled
            } else {
                TaskResult::Ready(output)
            };
            let _ = result_sender.send(result);
        }));

        if enqueue_result.is_err() {
            canceled.store(true, Ordering::Relaxed);
        }

        Task {
            receiver: result_receiver,
            canceled,
        }
    }
}

#[derive(Clone)]
pub struct UiExecutor {
    inner: Rc<UiExecutorInner>,
}

struct UiExecutorInner {
    queue: Mutex<VecDeque<Box<dyn FnOnce() + 'static>>>,
    wake_handle: Mutex<Option<WakeHandle>>,
}

impl UiExecutor {
    pub(super) fn new() -> Self {
        Self {
            inner: Rc::new(UiExecutorInner {
                queue: Mutex::new(VecDeque::new()),
                wake_handle: Mutex::new(None),
            }),
        }
    }

    pub(super) fn set_wake_handle(&self, wake_handle: Option<WakeHandle>) {
        *self.inner.wake_handle.lock() = wake_handle;
    }

    pub fn spawn<F>(&self, future: F) -> Task<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let (result_sender, result_receiver) = bounded(1);
        let canceled = Arc::new(AtomicBool::new(false));
        let canceled_for_job = canceled.clone();

        self.inner.queue.lock().push_back(Box::new(move || {
            if canceled_for_job.load(Ordering::Relaxed) {
                let _ = result_sender.send(TaskResult::Canceled);
                return;
            }

            let output = pollster::block_on(future);
            let result = if canceled_for_job.load(Ordering::Relaxed) {
                TaskResult::Canceled
            } else {
                TaskResult::Ready(output)
            };
            let _ = result_sender.send(result);
        }));

        if let Some(wake_handle) = self.inner.wake_handle.lock().clone() {
            wake_handle();
        }

        Task {
            receiver: result_receiver,
            canceled,
        }
    }

    pub fn run_pending(&self) {
        let mut pending = VecDeque::new();
        std::mem::swap(&mut pending, &mut *self.inner.queue.lock());
        while let Some(job) = pending.pop_front() {
            job();
        }
    }
}
