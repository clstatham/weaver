use std::fmt::Debug;

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};

use crate::id::{DynamicId, Registry};

pub trait Downcast: std::any::Any {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: std::any::Any> Downcast for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A component is a data structure that can be attached to an entity.
pub trait Component: Downcast + Send + Sync + 'static {}

/// A unique pointer to a type-erased component.
pub struct Data {
    id: DynamicId,
    type_name: String,
    field_name: Option<String>,
    data: Box<dyn Downcast + Send + Sync + 'static>,
}

impl Data {
    pub fn new<T: Component>(data: T, field_name: Option<&str>, registry: &Registry) -> Self {
        let id = registry.get_static::<T>();
        if id == registry.get_static::<Data>() {
            panic!("Cannot create a Data from a Data")
        }
        let data = Box::new(data);
        Self {
            data,
            id,
            type_name: registry.static_name::<T>(),
            field_name: field_name.map(|s| s.to_string()),
        }
    }

    #[inline]
    pub fn get_as<T: Component>(&self) -> Option<&T> {
        (*self.data).as_any().downcast_ref::<T>()
    }

    #[inline]
    pub fn get_as_mut<T: Component>(&mut self) -> Option<&mut T> {
        (*self.data).as_any_mut().downcast_mut::<T>()
    }

    #[inline]
    pub const fn id(&self) -> DynamicId {
        self.id
    }

    #[inline]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    #[inline]
    pub fn field_name(&self) -> Option<&str> {
        self.field_name.as_deref()
    }

    #[inline]
    pub fn name(&self) -> &str {
        match self.field_name() {
            Some(field_name) => field_name,
            None => self.type_name(),
        }
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("id", &self.id)
            .field("type_name", &self.type_name)
            .field("field_name", &self.field_name)
            .finish()
    }
}

pub struct LockedData {
    data: AtomicRefCell<Data>,
}

impl LockedData {
    pub fn new(data: Data) -> Self {
        Self {
            data: AtomicRefCell::new(data),
        }
    }

    #[inline]
    pub fn borrow(&self) -> AtomicRef<'_, Data> {
        self.data.borrow()
    }

    #[inline]
    pub fn borrow_mut(&self) -> AtomicRefMut<'_, Data> {
        self.data.borrow_mut()
    }

    pub fn into_inner(self) -> Data {
        self.data.into_inner()
    }
}
