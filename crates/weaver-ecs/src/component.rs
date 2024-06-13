use std::{
    any::TypeId,
    ops::{Deref, DerefMut},
};

use weaver_reflect_macros::reflect_trait;
use weaver_util::{
    lock::{ArcRead, ArcWrite, SharedLock},
    prelude::{anyhow, impl_downcast, DowncastSync},
    TypeIdMap,
};

use crate::{self as weaver_ecs};

#[reflect_trait]
pub trait Component: DowncastSync {}
impl_downcast!(sync Component);

#[reflect_trait]
pub trait Resource: DowncastSync {}
impl_downcast!(sync Resource);

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

#[derive(Default)]
pub struct Resources {
    resources: TypeIdMap<SharedLock<Box<dyn Resource>>>,
}

impl Resources {
    pub fn insert<T: Resource>(&mut self, resource: T) {
        self.resources
            .insert(TypeId::of::<T>(), SharedLock::new(Box::new(resource)));
    }

    pub fn get<T: Resource>(&self) -> Option<Res<T>> {
        self.resources
            .get(&TypeId::of::<T>())
            .map(|resource| Res::new(resource.read()))
    }

    pub fn get_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        self.resources
            .get(&TypeId::of::<T>())
            .map(|resource| ResMut::new(resource.write()))
    }

    pub fn remove<T: Resource>(&mut self) -> Option<T> {
        self.resources.remove(&TypeId::of::<T>()).map(|resource| {
            *resource
                .into_inner()
                .unwrap()
                .downcast()
                .map_err(|_| anyhow!("Failed to downcast resource"))
                .unwrap()
        })
    }

    pub fn contains<T: Resource>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }
}
