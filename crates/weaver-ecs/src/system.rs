use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::Arc,
};

use crate::{
    prelude::{Component, Mut, Query, Ref, World},
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
    fn run(&self, world: Rc<World>) -> anyhow::Result<()>;
}

pub trait SystemParam {
    fn fetch(world: Rc<World>) -> Option<Self>
    where
        Self: Sized;
}

impl<Q> SystemParam for Query<Q>
where
    Q: QueryFilter,
{
    fn fetch(world: Rc<World>) -> Option<Self> {
        Some(Query::new(world))
    }
}

pub struct Res<T: Component> {
    value: Ref<T>,
}

impl<T> Deref for Res<T>
where
    T: Component,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: Component> SystemParam for Res<T> {
    fn fetch(world: Rc<World>) -> Option<Self> {
        world.get_resource::<T>().map(|value| Self { value })
    }
}

pub struct ResMut<T: Component> {
    value: Mut<T>,
}

impl<T> Deref for ResMut<T>
where
    T: Component,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for ResMut<T>
where
    T: Component,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: Component> SystemParam for ResMut<T> {
    fn fetch(world: Rc<World>) -> Option<Self> {
        world.get_resource_mut::<T>().map(|value| Self { value })
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
                    fn run(&self, world: Rc<World>) -> anyhow::Result<()> {
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
    Func: Fn(Rc<World>) -> anyhow::Result<()> + 'static,
{
    fn into_system(self) -> Arc<dyn System> {
        struct FunctionSystemImpl<Func> {
            func: Func,
        }

        impl<Func> System for FunctionSystemImpl<Func>
        where
            Func: Fn(Rc<World>) -> anyhow::Result<()> + 'static,
        {
            fn run(&self, world: Rc<World>) -> anyhow::Result<()> {
                (self.func)(world)
            }
        }

        Arc::new(FunctionSystemImpl { func: self })
    }
}
