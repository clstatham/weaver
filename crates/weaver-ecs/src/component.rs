use std::{
    any::TypeId,
    ops::{Deref, DerefMut},
};

use weaver_reflect_macros::reflect_trait;
use weaver_util::{
    lock::{ArcRead, ArcWrite, SharedLock},
    prelude::{impl_downcast, DowncastSync},
    TypeIdMap,
};

use crate::{
    self as weaver_ecs,
    prelude::{
        ChangeDetection, ChangeDetectionMut, ComponentTicks, Reflect, Tick, Ticks, TicksMut,
    },
};

pub trait Component: DowncastSync + Reflect {}
impl_downcast!(Component);

#[reflect_trait]
pub trait Resource: DowncastSync {}
impl_downcast!(Resource);

pub struct Res<T: Resource> {
    value: ArcRead<Box<dyn Resource>>,
    ticks: Ticks,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Resource> Res<T> {
    pub(crate) fn new(value: ArcRead<Box<dyn Resource>>, ticks: Ticks) -> Self {
        Self {
            value,
            ticks,
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

impl<T: Resource> ChangeDetection for Res<T> {
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

pub struct ResMut<T: Resource> {
    value: ArcWrite<Box<dyn Resource>>,
    ticks: TicksMut,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Resource> ResMut<T> {
    pub(crate) fn new(value: ArcWrite<Box<dyn Resource>>, ticks: TicksMut) -> Self {
        Self {
            value,
            ticks,
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
        self.set_changed();
        (**self.value)
            .downcast_mut()
            .expect("Failed to downcast resource")
    }
}

impl<T> ChangeDetection for ResMut<T>
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

impl<T> ChangeDetectionMut for ResMut<T>
where
    T: Resource,
{
    type Inner = T;

    fn bypass_change_detection(&mut self) -> &mut Self::Inner {
        (**self.value)
            .downcast_mut()
            .expect("Failed to downcast resource")
    }

    fn set_changed(&mut self) {
        *self.ticks.changed = self.ticks.this_run;
    }
}

pub struct ResourceData {
    data: SharedLock<Box<dyn Resource>>,
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
            let _ = std::mem::replace(&mut *data.data.write(), Box::new(resource));
        } else {
            self.resources.insert(
                type_id,
                ResourceData {
                    data: SharedLock::new(Box::new(resource)),
                    added_tick: SharedLock::new(change_tick),
                    changed_tick: SharedLock::new(change_tick),
                },
            );
        }
        *self.resources.get(&type_id).unwrap().changed_tick.write() = change_tick;
    }

    pub fn get<T: Resource>(&self) -> Option<Res<T>> {
        self.resources.get(&TypeId::of::<T>()).map(|resource| {
            Res::new(
                resource.data.read_arc(),
                Ticks {
                    added: resource.added_tick.read_arc(),
                    changed: resource.changed_tick.read_arc(),
                    last_run: Tick::new(0),
                    this_run: Tick::new(0),
                },
            )
        })
    }

    pub fn get_mut<T: Resource>(&self, last_run: Tick, this_run: Tick) -> Option<ResMut<T>> {
        self.resources.get(&TypeId::of::<T>()).map(|resource| {
            ResMut::new(
                resource.data.write_arc(),
                TicksMut {
                    added: resource.added_tick.write_arc(),
                    changed: resource.changed_tick.write_arc(),
                    last_run,
                    this_run,
                },
            )
        })
    }

    pub fn remove<T: Resource>(&mut self) -> Option<(T, ComponentTicks)> {
        self.resources.remove(&TypeId::of::<T>()).map(|resource| {
            let data = resource.data.into_inner().unwrap();
            let added_tick = *resource.added_tick.read();
            let changed_tick = *resource.changed_tick.read();
            (
                *data.downcast().unwrap_or_else(|_| {
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
