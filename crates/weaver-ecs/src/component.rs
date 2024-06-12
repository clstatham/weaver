use std::any::TypeId;

use anyhow::anyhow;
use weaver_util::{
    lock::SharedLock,
    prelude::{impl_downcast, DowncastSync},
    TypeIdMap,
};

use crate::prelude::{Res, ResMut};

pub trait Component: DowncastSync {}
impl_downcast!(sync Component);

pub trait Resource: DowncastSync {}
impl_downcast!(sync Resource);

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
