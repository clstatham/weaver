use crate::task_pool::TaskPool;
use std::ops::Deref;
use std::sync::OnceLock;

macro_rules! task_pool {
    ($s:ident, $t:ident) => {
        static $s: OnceLock<$t> = OnceLock::new();

        pub struct $t(TaskPool);

        impl $t {
            pub fn get_or_init(f: impl FnOnce() -> TaskPool) -> &'static Self {
                $s.get_or_init(|| Self(f()))
            }

            pub fn try_get() -> Option<&'static Self> {
                $s.get()
            }

            pub fn get() -> &'static Self {
                $s.get()
                    .expect(concat!(stringify!($t), " is not initialized"))
            }
        }

        impl Deref for $t {
            type Target = TaskPool;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

task_pool!(GLOBAL_TASK_POOL, GlobalTaskPool);

pub fn tick_task_pools() {
    GLOBAL_TASK_POOL
        .get()
        .unwrap()
        .with_local_executor(|global_local_executor| {
            for _ in 0..100 {
                global_local_executor.try_tick();
            }
        });
}
