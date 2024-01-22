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
pub trait Component: Debug + Downcast + Send + Sync + 'static {
    #[allow(unused)]
    fn fields(&self, registry: &Registry) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![]
    }
}

impl Component for () {}
impl Component for bool {}
impl Component for u8 {}
impl Component for u16 {}
impl Component for u32 {}
impl Component for u64 {}
impl Component for u128 {}
impl Component for usize {}
impl Component for i8 {}
impl Component for i16 {}
impl Component for i32 {}
impl Component for i64 {}
impl Component for i128 {}
impl Component for isize {}
impl Component for f32 {}
impl Component for f64 {}
impl Component for String {}

#[derive(Debug)]
pub struct MetaComponent {
    pub type_id: DynamicId,
    pub type_name: String,
    pub field_name: Option<String>,
}

impl Component for MetaComponent {
    fn fields(&self, _registry: &Registry) -> Vec<Data> {
        vec![]
    }
}

/// A unique pointer to a type-erased component.
pub struct Data {
    type_id: DynamicId,
    type_name: String,
    field_name: Option<String>,
    data: Box<dyn Component>,
    fields: Vec<Data>,
}

impl Data {
    pub fn new<T: Component>(data: T, field_name: Option<&str>, registry: &Registry) -> Self {
        let type_id = registry.get_static::<T>();
        let data = Box::new(data);
        let fields = data.fields(registry);
        Self {
            data,
            type_id,
            type_name: registry.static_name::<T>(),
            field_name: field_name.map(|s| s.to_string()),
            fields,
        }
    }

    pub fn new_meta(
        field_name: Option<&str>,
        type_name: &str,
        fields: Vec<Data>,
        registry: &Registry,
    ) -> Self {
        let type_id = registry.get_named(type_name);
        Self {
            data: Box::new(MetaComponent {
                type_id,
                type_name: type_name.to_string(),
                field_name: field_name.map(|s| s.to_string()),
            }),
            type_id,
            type_name: type_name.to_string(),
            field_name: field_name.map(|s| s.to_string()),
            fields,
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
    pub const fn type_id(&self) -> DynamicId {
        self.type_id
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

    #[inline]
    pub fn data(&self) -> &dyn Component {
        &*self.data
    }

    #[inline]
    pub fn fields(&mut self) -> &[Data] {
        &self.fields
    }

    #[inline]
    pub fn field_by_id(&self, id: DynamicId) -> Option<&Data> {
        self.fields.iter().find(|field| field.type_id == id)
    }

    #[inline]
    pub fn field_by_name<'a>(&'a self, name: &str) -> Option<&'a Data> {
        self.fields
            .iter()
            .filter(|field| field.field_name().is_some())
            .find(|field| {
                if let Some(field_name) = field.field_name() {
                    field_name == name
                } else {
                    false
                }
            })
    }

    #[inline]
    pub fn field_by_name_mut<'a>(&'a mut self, name: &str) -> Option<&'a mut Data> {
        self.fields
            .iter_mut()
            .filter(|field| field.field_name().is_some())
            .find(|field| {
                if let Some(field_name) = field.field_name() {
                    field_name == name
                } else {
                    false
                }
            })
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("id", &self.type_id)
            .field("type_name", &self.type_name)
            .field("field_name", &self.field_name)
            .field("data", &format!("{:?}", self.data))
            .field("fields", &self.fields)
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
