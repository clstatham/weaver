use std::{num::NonZeroUsize, sync::Arc, thread::JoinHandle};

use async_io::block_on;
use futures_lite::FutureExt;

use crate::{Executor, LocalExecutor, Task, ThreadExecutor};

pub struct TaskPool {
    executor: Arc<Executor<'static>>,
    threads: Vec<JoinHandle<()>>,
    shutdown_tx: async_channel::Sender<()>,
}

impl TaskPool {
    thread_local! {
        static LOCAL_EXECUTOR: LocalExecutor<'static> = const { LocalExecutor::new() };
        static THREAD_EXECUTOR: Arc<ThreadExecutor<'static>> = Arc::new(ThreadExecutor::new());
    }

    pub fn get_thread_executor() -> Arc<ThreadExecutor<'static>> {
        Self::THREAD_EXECUTOR.with(|e| e.clone())
    }

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = async_channel::unbounded();
        let executor = Arc::new(Executor::new());
        let threads = (0..std::thread::available_parallelism()
            .map(NonZeroUsize::get)
            .unwrap_or(1))
            .map(|thread_id| {
                let executor = executor.clone();
                let shutdown_rx = shutdown_rx.clone();

                let thread_name = format!("weaver-task-worker-{}", thread_id);
                let thread_builder = std::thread::Builder::new().name(thread_name);
                thread_builder
                    .spawn(move || {
                        TaskPool::LOCAL_EXECUTOR.with(|local_executor| {
                            loop {
                                let res = std::panic::catch_unwind(|| {
                                    let tick_forever = async move {
                                        loop {
                                            local_executor.tick().await;
                                        }
                                    };
                                    block_on(executor.run(tick_forever.or(shutdown_rx.recv())))
                                });
                                if let Ok(value) = res {
                                    value.unwrap_err();
                                    break;
                                }
                            }
                        });
                    })
                    .expect("failed to spawn worker thread")
            })
            .collect();

        Self {
            executor,
            threads,
            shutdown_tx,
        }
    }

    pub fn thread_num(&self) -> usize {
        self.threads.len()
    }

    pub fn spawn<T: Send + 'static>(
        &self,
        task: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        self.executor.spawn(task)
    }

    pub fn spawn_local<T: 'static>(&self, task: impl Future<Output = T> + 'static) -> Task<T> {
        TaskPool::LOCAL_EXECUTOR.with(|local_executor| local_executor.spawn(task))
    }

    pub fn with_local_executor<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&LocalExecutor<'static>) -> R,
    {
        TaskPool::LOCAL_EXECUTOR.with(|local_executor| f(local_executor))
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        self.shutdown_tx.close();

        let panicking = std::thread::panicking();
        for thread in self.threads.drain(..) {
            let res = thread.join();
            if panicking {
                res.expect("worker thread panicked");
            }
        }
    }
}
