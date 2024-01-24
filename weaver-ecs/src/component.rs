use std::{fmt::Debug, sync::Arc};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};

use crate::registry::{DynamicId, Registry};

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
pub trait Component: Downcast + Send + Sync + 'static {
    #[allow(unused)]
    fn field_ids(registry: &Registry) -> Vec<DynamicId>
    where
        Self: Sized,
    {
        vec![]
    }

    #[allow(unused)]
    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data>
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

macro_rules! data_arithmetic {
    ($registry:ident, $lhs:ident, $op:tt, $rhs:ident; $($ty:ty),*) => {
        $(
            if let Some(lhs) = (&*$lhs).as_any().downcast_ref::<$ty>() {
                if let Some(rhs) = (&*$rhs).as_any().downcast_ref::<$ty>() {
                    return Ok(Data::new(*lhs $op *rhs, None, &$registry));
                }
            }
        )*
    };

    (ref mut $lhs:ident, $op:tt, $rhs:ident; $($ty:ty),*) => {
        $(
            if let Some(lhs) = (&mut *$lhs).as_any_mut().downcast_mut::<$ty>() {
                if let Some(rhs) = (&*$rhs).as_any().downcast_ref::<$ty>() {
                    *lhs $op *rhs;
                    return Ok(());
                }
            }
        )*
    };
}

macro_rules! data_display {
    ($this:ident, $type_name:expr, $f:ident; $($ty:ty),*) => {
        if true $( && (&*$this).as_any().downcast_ref::<$ty>().is_none())* {
            write!($f, "<{}>", $type_name).unwrap();
            return;
        }
        $(
            if let Some(lhs) = (&*$this).as_any().downcast_ref::<$ty>() {
                write!($f, "{}", lhs).unwrap();
            }
        )*
    };
}

#[derive(Clone)]
pub struct MetaData {
    pub(crate) type_id: DynamicId,
    pub(crate) type_name: String,
    pub(crate) fields: Vec<DynamicId>,
}

impl MetaData {
    pub fn new<T: Component>(registry: &Registry) -> Self {
        let type_id = registry.get_static::<T>();
        let fields = T::field_ids(registry);
        Self {
            type_id,
            type_name: registry.static_name::<T>(),
            fields,
        }
    }

    pub fn new_meta(type_name: &str, fields: Vec<DynamicId>, registry: &Registry) -> Self {
        let type_id = registry.get_named(type_name);
        Self {
            type_id,
            type_name: type_name.to_string(),
            fields,
        }
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
    pub fn fields(&self) -> &[DynamicId] {
        &self.fields
    }
}

/// A shared pointer to a type-erased component.
#[derive(Clone)]
pub struct Data {
    pub(crate) type_id: DynamicId,
    pub(crate) type_name: String,
    pub(crate) field_name: Option<String>,
    data: Arc<AtomicRefCell<dyn Component>>,
    pub(crate) fields: Vec<Data>,
    registry: Arc<Registry>,
}

impl Data {
    pub fn new<T: Component>(data: T, field_name: Option<&str>, registry: &Arc<Registry>) -> Self {
        let type_id = registry.get_static::<T>();
        let fields = data.fields(registry);
        let data = Arc::new(AtomicRefCell::new(data));
        Self {
            data,
            type_id,
            type_name: registry.static_name::<T>(),
            field_name: field_name.map(|s| s.to_string()),
            fields,
            registry: registry.clone(),
        }
    }

    pub fn new_dynamic(
        type_name: &str,
        field_name: Option<&str>,
        fields: Vec<Data>,
        registry: &Arc<Registry>,
    ) -> Self {
        let type_id = registry.get_named(type_name);
        Self {
            data: Arc::new(AtomicRefCell::new(())),
            type_id,
            type_name: type_name.to_string(),
            field_name: field_name.map(|s| s.to_string()),
            fields,
            registry: registry.clone(),
        }
    }

    #[inline]
    pub fn get_as<T: Component>(&self) -> AtomicRef<'_, T> {
        AtomicRef::map(self.data.borrow(), |data| {
            data.as_any().downcast_ref::<T>().unwrap()
        })
    }

    #[inline]
    pub fn get_as_mut<T: Component>(&self) -> AtomicRefMut<'_, T> {
        AtomicRefMut::map(self.data.borrow_mut(), |data| {
            data.as_any_mut().downcast_mut::<T>().unwrap()
        })
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
    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    #[inline]
    pub fn borrow(&self) -> AtomicRef<'_, dyn Component> {
        self.data.borrow()
    }

    #[inline]
    pub fn borrow_mut(&self) -> AtomicRefMut<'_, dyn Component> {
        self.data.borrow_mut()
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

    pub fn display(&self, f: &mut std::fmt::Formatter<'_>) {
        let this = self.borrow();
        let type_name = self.type_name();
        data_display!(this, type_name, f; bool, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, String);
    }

    pub fn add(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, +, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn sub(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, -, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn mul(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, *, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn div(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, /, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn rem(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, %, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn assign(&self, rhs: &Data) -> anyhow::Result<()> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let mut lhs = self.borrow_mut();
        let rhs = rhs.borrow();

        data_arithmetic!(ref mut lhs, =, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn add_assign(&self, rhs: &Data) -> anyhow::Result<()> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }
        let mut lhs = self.borrow_mut();
        let rhs = rhs.borrow();

        data_arithmetic!(ref mut lhs, +=, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn sub_assign(&self, rhs: &Data) -> anyhow::Result<()> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }
        let mut lhs = self.borrow_mut();
        let rhs = rhs.borrow();

        data_arithmetic!(ref mut lhs, -=, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn mul_assign(&self, rhs: &Data) -> anyhow::Result<()> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let mut lhs = self.borrow_mut();
        let rhs = rhs.borrow();

        data_arithmetic!(ref mut lhs, *=, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn div_assign(&self, rhs: &Data) -> anyhow::Result<()> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let mut lhs = self.borrow_mut();
        let rhs = rhs.borrow();

        data_arithmetic!(ref mut lhs, /=, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn rem_assign(&self, rhs: &Data) -> anyhow::Result<()> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let mut lhs = self.borrow_mut();
        let rhs = rhs.borrow();

        data_arithmetic!(ref mut lhs, %=, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

        Err(anyhow::anyhow!("Type mismatch"))
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("id", &self.type_id)
            .field("type_name", &self.type_name)
            .field("field_name", &self.field_name)
            .field("fields", &self.fields)
            .finish()
    }
}
