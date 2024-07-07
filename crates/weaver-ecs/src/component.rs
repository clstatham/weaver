use std::{
    any::TypeId,
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

use weaver_reflect_macros::reflect_trait;
use weaver_util::{
    lock::SharedLock,
    prelude::{impl_downcast, Downcast},
    TypeIdMap,
};

use crate::{
    self as weaver_ecs,
    prelude::{ChangeDetection, ChangeDetectionMut, ComponentTicks, Tick, Ticks, TicksMut},
};

#[reflect_trait]
pub trait Component: Downcast {}
impl_downcast!(Component);

#[reflect_trait]
pub trait Resource: Downcast {}
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
