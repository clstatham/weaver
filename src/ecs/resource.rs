use std::sync::{RwLockReadGuard, RwLockWriteGuard};

use super::component::Downcast;

pub trait Resource: Downcast + 'static {
    fn resource_id() -> u64
    where
        Self: Sized;
}

pub struct Res<'a, T: Resource> {
    resource: RwLockReadGuard<'a, dyn Resource>,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T: Resource> Res<'a, T> {
    pub fn new(resource: RwLockReadGuard<'a, dyn Resource>) -> Self {
        Self {
            resource,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, T: Resource> std::ops::Deref for Res<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        (*self.resource)
            .as_any()
            .downcast_ref::<T>()
            .expect("BUG: Failed to downcast resource")
    }
}

pub struct ResMut<'a, T: Resource> {
    resource: RwLockWriteGuard<'a, dyn Resource>,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T: Resource> ResMut<'a, T> {
    pub fn new(resource: RwLockWriteGuard<'a, dyn Resource>) -> Self {
        Self {
            resource,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, T: Resource> std::ops::Deref for ResMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        (*self.resource).as_any().downcast_ref::<T>().unwrap()
    }
}

impl<'a, T: Resource> std::ops::DerefMut for ResMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        (*self.resource).as_any_mut().downcast_mut::<T>().unwrap()
    }
}
