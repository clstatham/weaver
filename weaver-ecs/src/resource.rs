use std::sync::RwLockReadGuard;

use crate::component::Downcast;

pub trait Resource: Downcast + Send + Sync + 'static {
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
        (*self.resource).as_any().downcast_ref::<T>().unwrap()
    }
}
