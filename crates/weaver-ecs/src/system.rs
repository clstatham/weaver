use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use weaver_util::lock::{ArcRead, ArcWrite};

use crate::{
    prelude::{Query, Resource, World},
    query::QueryFilter,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemStage {
    PreInit,
    Init,
    PostInit,

    PreUpdate,
    Update,
    PostUpdate,

    PreUi,
    Ui,
    PostUi,

    PreRender,
    Render,
    PostRender,

    PreShutdown,
    Shutdown,
    PostShutdown,
}

pub trait System: 'static {
    fn run(&self, world: Arc<World>) -> anyhow::Result<()>;
}

pub trait SystemParam {
    fn fetch(world: Arc<World>) -> Option<Self>
    where
        Self: Sized;
}

impl<Q> SystemParam for Query<Q>
where
    Q: QueryFilter,
{
    fn fetch(world: Arc<World>) -> Option<Self> {
        Some(Query::new(world))
    }
}

pub struct Res<T: Resource> {
    value: ArcRead<Box<dyn Resource>>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Resource> Res<T> {
    pub fn new(value: ArcRead<Box<dyn Resource>>) -> Self {
        Self {
            value,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Deref for Res<T>
where
    T: Resource,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        (**self.value)
            .downcast_ref()
            .expect("Failed to downcast resource")
    }
}

impl<T: Resource> SystemParam for Res<T> {
    fn fetch(world: Arc<World>) -> Option<Self> {
        world.get_resource::<T>()
    }
}

pub struct ResMut<T: Resource> {
    value: ArcWrite<Box<dyn Resource>>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Resource> ResMut<T> {
    pub fn new(value: ArcWrite<Box<dyn Resource>>) -> Self {
        Self {
            value,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Deref for ResMut<T>
where
    T: Resource,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        (**self.value)
            .downcast_ref()
            .expect("Failed to downcast resource")
    }
}

impl<T> DerefMut for ResMut<T>
where
    T: Resource,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        (**self.value)
            .downcast_mut()
            .expect("Failed to downcast resource")
    }
}

impl<T: Resource> SystemParam for ResMut<T> {
    fn fetch(world: Arc<World>) -> Option<Self> {
        world.get_resource_mut::<T>()
    }
}

pub trait FunctionSystem<Marker> {
    fn into_system(self) -> Arc<dyn System>;
}

macro_rules! impl_function_system {
    ($($param:ident),*) => {
        impl<Func, $($param),*> FunctionSystem<fn($($param),*)> for Func
        where
            Func: Fn($($param),*) -> anyhow::Result<()> + 'static,
            $($param: SystemParam + 'static),*
        {

            #[allow(unused_parens, non_snake_case)]
            fn into_system(self) -> Arc<dyn System> {
                struct FunctionSystemImpl<Func, $($param),*> {
                    func: Func,
                    _marker: std::marker::PhantomData<($($param),*)>,
                }

                impl <Func, $($param),*> System for FunctionSystemImpl<Func, $($param),*>
                where
                    Func: Fn($($param),*) -> anyhow::Result<()> + 'static,
                    $($param: SystemParam + 'static),*
                {
                    fn run(&self, world: Arc<World>) -> anyhow::Result<()> {
                        let ($($param),*) = ($($param::fetch(world.clone()).ok_or_else(|| anyhow::anyhow!("Failed to fetch system param"))?),*);
                        (self.func)($($param),*)
                    }
                }

                Arc::new(FunctionSystemImpl {
                    func: self,
                    _marker: std::marker::PhantomData,
                })
            }
        }
    };
}

impl_function_system!(A);
impl_function_system!(A, B);
impl_function_system!(A, B, C);
impl_function_system!(A, B, C, D);
impl_function_system!(A, B, C, D, E);
impl_function_system!(A, B, C, D, E, F);
impl_function_system!(A, B, C, D, E, F, G);
impl_function_system!(A, B, C, D, E, F, G, H);

impl<Func> FunctionSystem<()> for Func
where
    Func: Fn(Arc<World>) -> anyhow::Result<()> + 'static,
{
    fn into_system(self) -> Arc<dyn System> {
        struct FunctionSystemImpl<Func> {
            func: Func,
        }

        impl<Func> System for FunctionSystemImpl<Func>
        where
            Func: Fn(Arc<World>) -> anyhow::Result<()> + 'static,
        {
            fn run(&self, world: Arc<World>) -> anyhow::Result<()> {
                (self.func)(world)
            }
        }

        Arc::new(FunctionSystemImpl { func: self })
    }
}
