use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

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
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        std::any::type_name::<Self>()
    }

    #[allow(unused)]
    fn field_ids(registry: &Registry) -> Vec<DynamicId>
    where
        Self: Sized,
    {
        vec![]
    }

    #[allow(unused)]
    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data> {
        vec![]
    }

    #[allow(unused)]
    fn register_methods(registry: &Arc<Registry>)
    where
        Self: Sized,
    {
    }

    #[allow(unused)]
    fn into_dynamic_data(self, field_name: Option<&str>, registry: &Arc<Registry>) -> Data
    where
        Self: Sized,
    {
        Self::register_methods(registry);
        Data::new_dynamic(
            registry.static_name::<Self>(),
            field_name,
            impls_clone::<Self>(),
            self.fields(registry),
            registry,
        )
    }
}

pub trait Method: Send + Sync + 'static {
    fn call(&self, args: &[&Data]) -> anyhow::Result<Data>;
}

impl<F> Method for F
where
    F: Fn(&[&Data]) -> anyhow::Result<Data> + Send + Sync + 'static,
{
    fn call(&self, args: &[&Data]) -> anyhow::Result<Data> {
        self(args)
    }
}

#[derive(Clone, Copy)]
pub enum MethodArgType {
    Mut(DynamicId),
    Ref(DynamicId),
    Owned(DynamicId),
}

impl MethodArgType {
    pub fn type_id(&self) -> DynamicId {
        match self {
            Self::Mut(id) | Self::Ref(id) | Self::Owned(id) => *id,
        }
    }

    pub fn is_mut(&self) -> bool {
        matches!(self, Self::Mut(_))
    }

    pub fn is_ref(&self) -> bool {
        matches!(self, Self::Ref(_))
    }

    pub fn is_owned(&self) -> bool {
        matches!(self, Self::Owned(_))
    }
}

pub struct MethodWrapper {
    name: String,
    method: Arc<dyn Method>,
    arg_types: Vec<MethodArgType>,
    return_type: Option<MethodArgType>,
}

impl MethodWrapper {
    pub fn new(
        name: &str,
        arg_types: impl IntoIterator<Item = MethodArgType>,
        return_type: Option<MethodArgType>,
        method: Arc<dyn Method>,
    ) -> Self {
        Self {
            name: name.to_string(),
            method,
            arg_types: arg_types.into_iter().collect(),
            return_type,
        }
    }

    pub fn from_method<F: Method + Send + Sync + 'static>(
        name: &str,
        arg_types: impl IntoIterator<Item = MethodArgType>,
        return_type: Option<MethodArgType>,
        f: F,
    ) -> Self {
        Self::new(name, arg_types, return_type, Arc::new(f))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn num_args(&self) -> usize {
        self.arg_types.len()
    }

    pub fn arg_types(&self) -> &[MethodArgType] {
        &self.arg_types
    }

    pub fn call(&self, args: &[&Data]) -> anyhow::Result<Data> {
        if args.len() != self.num_args() {
            return Err(anyhow::anyhow!(
                "Incorrect number of arguments: {} (expected {})",
                args.len(),
                self.num_args()
            ));
        }
        for (arg, arg_type) in args.iter().zip(self.arg_types()) {
            if arg.type_id() != arg_type.type_id() {
                return Err(anyhow::anyhow!(
                    "Incorrect argument type: {} (expected {})",
                    arg.type_id(),
                    arg_type.type_id()
                ));
            }
        }
        let result = self.method.call(args)?;
        if let Some(return_type) = self.return_type {
            if result.type_id() != return_type.type_id() {
                return Err(anyhow::anyhow!(
                    "Incorrect return type: {} (expected {})",
                    result.type_id(),
                    return_type.type_id()
                ));
            }
        }
        Ok(result)
    }
}

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

macro_rules! try_all_types {
    ($this:ident; $($ty:ty),*; $main:block else $els:block) => {
        if true $( && (&*$this).as_any().downcast_ref::<$ty>().is_none())* {
            $els
        }
        $(
            if let Some($this) = (&*$this).as_any().downcast_ref::<$ty>() {
                $main
            }
        )*
    };
}
pub const fn impls_clone<T: ?Sized>() -> bool {
    impls::impls!(T: Clone)
}

/// A shared pointer to a type-erased component.
#[derive(Clone)]
pub struct Data {
    pub(crate) type_id: DynamicId,
    pub(crate) type_name: String,
    pub(crate) field_name: Option<String>,
    data: Arc<RwLock<dyn Component>>,
    fields: Vec<Data>,
    registry: Arc<Registry>,
    is_clone: bool,
    is_dynamic: bool,
}

impl Data {
    pub fn new<T: Component>(data: T, field_name: Option<&str>, registry: &Arc<Registry>) -> Self {
        let type_id = registry.get_static::<T>();
        let fields = data.fields(registry);
        T::register_methods(registry);
        let data = Arc::new(RwLock::new(data));
        Self {
            data,
            type_id,
            type_name: registry.static_name::<T>().to_string(),
            field_name: field_name.map(|s| s.to_string()),
            fields,
            registry: registry.clone(),
            is_clone: impls_clone::<T>(),
            is_dynamic: false,
        }
    }

    pub fn new_dynamic(
        type_name: &str,
        field_name: Option<&str>,
        is_clone: bool,
        fields: Vec<Data>,
        registry: &Arc<Registry>,
    ) -> Self {
        let type_id = registry.get_named(type_name);
        Self {
            data: Arc::new(RwLock::new(())),
            type_id,
            type_name: type_name.to_string(),
            field_name: field_name.map(|s| s.to_string()),
            fields,
            registry: registry.clone(),
            is_clone,
            is_dynamic: true,
        }
    }

    #[inline]
    pub fn get_as<T: Component>(&self) -> MappedRwLockReadGuard<'_, T> {
        if self.is_dynamic {
            panic!("Cannot get dynamic component as immutable");
        }
        RwLockReadGuard::map(self.data.read(), |data| {
            data.as_any().downcast_ref::<T>().unwrap()
        })
    }

    #[inline]
    pub fn get_as_mut<T: Component>(&self) -> MappedRwLockWriteGuard<'_, T> {
        if self.is_dynamic {
            panic!("Cannot get dynamic component as mutable");
        }
        RwLockWriteGuard::map(self.data.write(), |data| {
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
    pub const fn is_clone(&self) -> bool {
        self.is_clone
    }

    #[inline]
    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    #[inline]
    pub fn borrow(&self) -> RwLockReadGuard<'_, dyn Component> {
        self.data.read()
    }

    #[inline]
    pub fn borrow_mut(&self) -> RwLockWriteGuard<'_, dyn Component> {
        self.data.write()
    }

    #[inline]
    pub fn fields(&self) -> Vec<Data> {
        if self.is_dynamic {
            return self.fields.to_owned();
        }

        let data = self.borrow();
        data.fields(&self.registry)
    }

    #[inline]
    pub fn field_by_id(&self, id: DynamicId) -> Option<Data> {
        self.fields()
            .iter()
            .find(|field| field.type_id == id)
            .cloned()
    }

    #[inline]
    pub fn field_by_name(&self, name: &str) -> Option<Data> {
        self.fields()
            .iter()
            .filter(|field| field.field_name().is_some())
            .find(|field| {
                if let Some(field_name) = field.field_name() {
                    field_name == name
                } else {
                    false
                }
            })
            .cloned()
    }

    #[inline]
    pub fn method_by_name(&self, name: &str) -> Option<Arc<MethodWrapper>> {
        self.registry.method_by_id(self.type_id, name)
    }

    #[inline]
    pub fn call_method(&self, name: &str, args: &[&Data]) -> anyhow::Result<Data> {
        if let Some(method) = self.method_by_name(name) {
            return method.call(args);
        }
        Err(anyhow::anyhow!("Method does not exist"))
    }

    #[inline]
    pub fn clone_as<T: Component + Clone>(&self) -> Option<Self> {
        let data = self.data.read();
        let data = (*data).as_any().downcast_ref::<T>()?;
        Some(Self::new(
            data.clone(),
            self.field_name.as_deref(),
            &self.registry,
        ))
    }

    pub fn try_clone(&self) -> Option<Self> {
        if !self.is_clone {
            return None;
        }
        let this = self.borrow();
        try_all_types!(this; bool, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, String; {
            return Some(Self::new(this.clone(), self.field_name.as_deref(), &self.registry));
        } else {
            return None;
        });
        None
    }

    pub fn display(&self, f: &mut std::fmt::Formatter<'_>) {
        let fields = self.fields();
        let component = self.borrow();
        let type_name = self.type_name();
        try_all_types!(component; bool, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, String, glam::Vec2, glam::Vec3, glam::Vec4, glam::Mat2, glam::Mat3, glam::Mat4, glam::Quat; {
            write!(f, "{}", *component).unwrap();
            return;
        } else {
            write!(f, "{}", type_name).unwrap();
            if fields.is_empty() {
                return;
            }
            write!(f, " {{ ").unwrap();
            for field in &fields[..fields.len() - 1] {
                write!(f, "{} = ", field.field_name().unwrap()).unwrap();
                field.display(f);
                write!(f, ", ").unwrap();
            }
            write!(f, "{} = ", fields[fields.len() - 1].field_name().unwrap()).unwrap();
            fields[fields.len() - 1].display(f);
            write!(f, " }}").unwrap();

            return;
        });
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

    pub fn lt(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, <, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn le(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, <=, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn gt(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, >, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn ge(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Err(anyhow::anyhow!("Type mismatch"));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, >=, rhs; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn eq(&self, rhs: &Data) -> anyhow::Result<Data> {
        if self.type_id != rhs.type_id {
            return Ok(Data::new(false, None, &self.registry));
        }

        let reg = self.registry.clone();

        let lhs = self.borrow();
        let rhs = rhs.borrow();

        data_arithmetic!(reg, lhs, ==, rhs; bool, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, String);

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn and(&self, rhs: &Data) -> anyhow::Result<Data> {
        let lhs = self.borrow();
        let rhs = rhs.borrow();

        if let Some(lhs) = (*lhs).as_any().downcast_ref::<bool>() {
            if let Some(rhs) = (*rhs).as_any().downcast_ref::<bool>() {
                return Ok(Data::new(*lhs && *rhs, None, &self.registry));
            }
        }

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn or(&self, rhs: &Data) -> anyhow::Result<Data> {
        let lhs = self.borrow();
        let rhs = rhs.borrow();

        if let Some(lhs) = (*lhs).as_any().downcast_ref::<bool>() {
            if let Some(rhs) = (*rhs).as_any().downcast_ref::<bool>() {
                return Ok(Data::new(*lhs || *rhs, None, &self.registry));
            }
        }

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn not(&self) -> anyhow::Result<Data> {
        let this = self.borrow();

        if let Some(this) = (*this).as_any().downcast_ref::<bool>() {
            return Ok(Data::new(!*this, None, &self.registry));
        }

        Err(anyhow::anyhow!("Type mismatch"))
    }

    pub fn xor(&self, rhs: &Data) -> anyhow::Result<Data> {
        let lhs = self.borrow();
        let rhs = rhs.borrow();

        if let Some(lhs) = (*lhs).as_any().downcast_ref::<bool>() {
            if let Some(rhs) = (*rhs).as_any().downcast_ref::<bool>() {
                return Ok(Data::new(*lhs ^ *rhs, None, &self.registry));
            }
        }

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
            .field("fields", &self.fields())
            .finish()
    }
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.display(f);
        Ok(())
    }
}
