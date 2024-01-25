use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

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
    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data> {
        vec![]
    }

    #[allow(unused)]
    fn methods(&self, registry: &Arc<Registry>) -> Vec<MethodWrapper> {
        vec![]
    }

    #[allow(unused)]
    fn into_dynamic_data(self, field_name: Option<&str>, registry: &Arc<Registry>) -> Data
    where
        Self: Sized,
    {
        Data::new_dynamic(
            registry.static_name::<Self>().as_str(),
            field_name,
            impls_clone::<Self>(),
            self.fields(registry),
            self.methods(registry),
            registry,
        )
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

#[derive(Clone)]
pub struct MethodWrapper {
    name: String,
    method: Arc<dyn Method>,
    num_args: usize,
}

impl MethodWrapper {
    pub fn new(name: &str, num_args: usize, method: Arc<dyn Method>) -> Self {
        Self {
            name: name.to_string(),
            num_args,
            method,
        }
    }

    pub fn from_method<F: Method + Send + Sync + 'static>(
        name: &str,
        num_args: usize,
        f: F,
    ) -> Self {
        Self::new(name, num_args, Arc::new(f))
    }

    pub fn num_args(&self) -> usize {
        self.num_args
    }

    pub fn call(&self, args: &[&Data]) -> anyhow::Result<Data> {
        if args.len() != self.num_args {
            return Err(anyhow::anyhow!(
                "Incorrect number of arguments: {} (expected {})",
                args.len(),
                self.num_args
            ));
        }
        self.method.call(args)
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
    data: Arc<AtomicRefCell<dyn Component>>,
    fields: Vec<Data>,
    methods: Vec<MethodWrapper>,
    registry: Arc<Registry>,
    is_clone: bool,
    is_dynamic: bool,
}

impl Data {
    pub fn new<T: Component>(data: T, field_name: Option<&str>, registry: &Arc<Registry>) -> Self {
        let type_id = registry.get_static::<T>();
        let fields = data.fields(registry);
        let methods = data.methods(registry);
        let data = Arc::new(AtomicRefCell::new(data));
        Self {
            data,
            type_id,
            type_name: registry.static_name::<T>(),
            field_name: field_name.map(|s| s.to_string()),
            fields,
            registry: registry.clone(),
            is_clone: impls_clone::<T>(),
            methods,
            is_dynamic: false,
        }
    }

    pub fn new_dynamic(
        type_name: &str,
        field_name: Option<&str>,
        is_clone: bool,
        fields: Vec<Data>,
        methods: Vec<MethodWrapper>,
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
            is_clone,
            methods,
            is_dynamic: true,
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
    pub const fn is_clone(&self) -> bool {
        self.is_clone
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
    pub fn method_by_name(&self, name: &str) -> Option<&Arc<dyn Method>> {
        self.methods.iter().find_map(|method| {
            if method.name == name {
                Some(&method.method)
            } else {
                None
            }
        })
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
        let data = self.data.borrow();
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
            .field(
                "methods",
                &self
                    .methods
                    .iter()
                    .map(|method| method.name.clone())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.display(f);
        Ok(())
    }
}
