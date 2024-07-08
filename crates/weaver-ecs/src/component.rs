use std::{
    any::{Any, TypeId},
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

use weaver_reflect_macros::reflect_trait;
use weaver_util::{
    lock::SharedLock,
    prelude::{impl_downcast, DowncastSync},
    TypeIdMap,
};

use crate::{
    self as weaver_ecs,
    prelude::{ChangeDetection, ChangeDetectionMut, ComponentTicks, Tick, Ticks, TicksMut, World},
};

#[reflect_trait]
pub trait Component: DowncastSync {}
impl_downcast!(Component);

#[reflect_trait]
pub trait Resource: DowncastSync {}
impl_downcast!(Resource);

pub struct Res<'r, T: Resource> {
    pub(crate) value: &'r T,
    pub(crate) ticks: Ticks<'r>,
}

impl<'r, T: Resource> Res<'r, T> {
    #[inline]
    pub fn into_inner(self) -> &'r T {
        self.value
    }
}

impl<'r, T> Deref for Res<'r, T>
where
    T: Resource,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'r, T: Resource> ChangeDetection for Res<'r, T> {
    fn is_added(&self) -> bool {
        self.ticks
            .added
            .is_newer_than(self.ticks.last_run, self.ticks.this_run)
    }

    fn is_changed(&self) -> bool {
        self.ticks
            .changed
            .is_newer_than(self.ticks.last_run, self.ticks.this_run)
    }

    fn last_changed(&self) -> Tick {
        *self.ticks.changed
    }
}

pub struct ResMut<'r, T: Resource> {
    pub(crate) value: &'r mut T,
    pub(crate) ticks: TicksMut<'r>,
}

impl<'r, T: Resource> ResMut<'r, T> {
    #[inline]
    pub fn into_inner(self) -> &'r mut T {
        self.value
    }
}

impl<'r, T> Deref for ResMut<'r, T>
where
    T: Resource,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'r, T> DerefMut for ResMut<'r, T>
where
    T: Resource,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        self.value
    }
}

impl<'r, T> ChangeDetection for ResMut<'r, T>
where
    T: Resource,
{
    fn is_added(&self) -> bool {
        self.ticks
            .added
            .is_newer_than(self.ticks.last_run, self.ticks.this_run)
    }

    fn is_changed(&self) -> bool {
        self.ticks
            .changed
            .is_newer_than(self.ticks.last_run, self.ticks.this_run)
    }

    fn last_changed(&self) -> Tick {
        *self.ticks.changed
    }
}

impl<'r, T> ChangeDetectionMut for ResMut<'r, T>
where
    T: Resource,
{
    type Inner = T;

    fn bypass_change_detection(&mut self) -> &mut Self::Inner {
        self.value
    }

    fn set_changed(&mut self) {
        *self.ticks.changed = self.ticks.this_run;
    }
}

pub trait MultiResource<'w> {
    type Output;
    fn is_unique() -> bool;
    fn fetch(world: &'w mut World) -> Self::Output
    where
        Self: Sized;
}

impl<'w, T> MultiResource<'w> for T
where
    T: Resource,
{
    type Output = ResMut<'w, T>;

    fn is_unique() -> bool {
        true
    }

    fn fetch(world: &'w mut World) -> ResMut<'w, T> {
        let cell = world.as_unsafe_world_cell();
        unsafe { cell.get_resource_mut::<T>().unwrap() }
    }
}

macro_rules! impl_multi_resource_tuple {
    ($($name:ident),*) => {
        #[allow(unused_parens)]
        impl<'w, $($name: Resource),*> MultiResource<'w> for ($($name,)*) {
            type Output = ($(ResMut<'w, $name>),*);

            fn is_unique() -> bool {
                let mut set = std::collections::HashSet::new();
                $((set.insert(std::any::TypeId::of::<$name>())) &&)* true
            }

            fn fetch(world: &'w mut World) -> ($(ResMut<'w, $name>),*) {
                if !Self::is_unique() {
                    panic!("duplicate resource types");
                }

                let cell = world.as_unsafe_world_cell();

                unsafe {
                    ($(
                        cell.get_resource_mut::<$name>().unwrap()
                    ),*)
                }
            }
        }
    };
}

impl_multi_resource_tuple!(A);
impl_multi_resource_tuple!(A, B);
impl_multi_resource_tuple!(A, B, C);
impl_multi_resource_tuple!(A, B, C, D);
impl_multi_resource_tuple!(A, B, C, D, E);
impl_multi_resource_tuple!(A, B, C, D, E, F);
impl_multi_resource_tuple!(A, B, C, D, E, F, G);
impl_multi_resource_tuple!(A, B, C, D, E, F, G, H);

pub struct ResourceData {
    data: UnsafeCell<Box<dyn Resource>>,
    added_tick: SharedLock<Tick>,
    changed_tick: SharedLock<Tick>,
}

#[derive(Default)]
pub struct Resources {
    resources: TypeIdMap<ResourceData>,
}

impl Resources {
    pub fn insert<T: Resource>(&mut self, resource: T, change_tick: Tick) {
        let type_id = TypeId::of::<T>();
        if let Some(data) = self.resources.get_mut(&type_id) {
            let _ = std::mem::replace(&mut data.data, UnsafeCell::new(Box::new(resource)));
        } else {
            self.resources.insert(
                type_id,
                ResourceData {
                    data: UnsafeCell::new(Box::new(resource)),
                    added_tick: SharedLock::new(change_tick),
                    changed_tick: SharedLock::new(change_tick),
                },
            );
        }
        *self.resources.get(&type_id).unwrap().changed_tick.write() = change_tick;
    }

    pub fn get<T: Resource>(&self, last_run: Tick, this_run: Tick) -> Option<Res<'_, T>> {
        self.resources.get(&TypeId::of::<T>()).map(|resource| {
            let ticks = Ticks {
                added: resource.added_tick.read(),
                changed: resource.changed_tick.read(),
                last_run,
                this_run,
            };
            Res {
                value: unsafe { &*resource.data.get() }.downcast_ref().unwrap(),
                ticks,
            }
        })
    }

    pub fn get_mut<T: Resource>(
        &mut self,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<ResMut<'_, T>> {
        self.resources.get_mut(&TypeId::of::<T>()).map(|resource| {
            let ticks = TicksMut {
                added: resource.added_tick.write(),
                changed: resource.changed_tick.write(),
                last_run,
                this_run,
            };
            ResMut {
                value: unsafe { &mut *resource.data.get() }.downcast_mut().unwrap(),
                ticks,
            }
        })
    }

    pub fn remove<T: Resource>(&mut self) -> Option<(T, ComponentTicks)> {
        self.resources.remove(&TypeId::of::<T>()).map(|resource| {
            let added_tick = *resource.added_tick.read();
            let changed_tick = *resource.changed_tick.read();
            (
                *resource.data.into_inner().downcast().unwrap_or_else(|_| {
                    panic!(
                        "Failed to downcast resource: {}",
                        std::any::type_name::<T>()
                    )
                }),
                ComponentTicks {
                    added: added_tick,
                    changed: changed_tick,
                },
            )
        })
    }

    pub fn contains<T: Resource>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }
}

pub struct NonSend<'r, T: 'static> {
    pub(crate) value: &'r T,
    _marker: std::marker::PhantomData<*const ()>,
}

impl<'r, T: 'static> NonSend<'r, T> {
    #[inline]
    pub fn into_inner(self) -> &'r T {
        self.value
    }
}

impl<'r, T> Deref for NonSend<'r, T>
where
    T: 'static,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

pub struct NonSendResourceData {
    data: UnsafeCell<Box<dyn Any>>,
}

#[derive(Default)]
pub struct NonSendResources {
    resources: TypeIdMap<NonSendResourceData>,
}

impl NonSendResources {
    pub fn insert<T: 'static>(&mut self, resource: T) {
        let type_id = TypeId::of::<T>();
        if let Some(data) = self.resources.get_mut(&type_id) {
            let _ = std::mem::replace(&mut data.data, UnsafeCell::new(Box::new(resource)));
        } else {
            self.resources.insert(
                type_id,
                NonSendResourceData {
                    data: UnsafeCell::new(Box::new(resource)),
                },
            );
        }
    }

    pub fn get<T: 'static>(&self) -> Option<NonSend<'_, T>> {
        self.resources
            .get(&TypeId::of::<T>())
            .map(|resource| NonSend {
                value: unsafe { &*resource.data.get() }.downcast_ref().unwrap(),
                _marker: std::marker::PhantomData,
            })
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.resources
            .get_mut(&TypeId::of::<T>())
            .map(|resource| unsafe { &mut *resource.data.get() }.downcast_mut().unwrap())
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.resources.remove(&TypeId::of::<T>()).map(|resource| {
            *resource.data.into_inner().downcast().unwrap_or_else(|_| {
                panic!(
                    "Failed to downcast resource: {}",
                    std::any::type_name::<T>()
                )
            })
        })
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }
}
