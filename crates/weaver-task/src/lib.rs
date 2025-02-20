use std::{pin::Pin, thread::ThreadId};

pub mod futures;
pub mod task_pool;
pub mod usages;

pub use futures_lite;

pub trait SendFuture: Future + Send {}
impl<T: Future + Send> SendFuture for T {}

pub type BoxFuture<'a, T> = Pin<Box<dyn SendFuture<Output = T> + 'a>>;

pub type Executor<'a> = async_executor::Executor<'a>;
pub type LocalExecutor<'a> = async_executor::LocalExecutor<'a>;

pub type Task<T> = async_executor::Task<T>;
pub type FallibleTask<T> = async_executor::FallibleTask<T>;

pub struct ThreadExecutor<'task> {
    executor: Executor<'task>,
    thread_id: ThreadId,
}

impl Default for ThreadExecutor<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'task> ThreadExecutor<'task> {
    pub fn new() -> Self {
        Self {
            executor: Executor::new(),
            thread_id: std::thread::current().id(),
        }
    }

    pub fn spawn<T: Send + 'task>(&self, task: impl Future<Output = T> + Send + 'task) -> Task<T> {
        self.executor.spawn(task)
    }

    pub fn ticker<'ticker>(&'ticker self) -> Option<ThreadExecutorTicker<'task, 'ticker>> {
        if std::thread::current().id() == self.thread_id {
            Some(ThreadExecutorTicker {
                executor: self,
                _marker: std::marker::PhantomData,
            })
        } else {
            None
        }
    }

    pub fn is_same(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

pub struct ThreadExecutorTicker<'task, 'ticker> {
    executor: &'ticker ThreadExecutor<'task>,
    _marker: std::marker::PhantomData<*const ()>, // not Send/Sync
}

impl ThreadExecutorTicker<'_, '_> {
    pub async fn tick(&self) {
        self.executor.executor.tick().await;
    }

    pub fn try_tick(&self) -> bool {
        self.executor.executor.try_tick()
    }
}
