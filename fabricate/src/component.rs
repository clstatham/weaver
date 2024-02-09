use crate::{self as fabricate, registry::StaticId};
use std::{any::Any, collections::HashMap};

use anyhow::Result;

use crate::prelude::*;

pub enum TakesSelf {
    None,
    Ref,
    RefMut,
}

pub enum MethodArg<'a> {
    Ref(Ref<'a>),
    Mut(Mut<'a>),
    Owned(Data),
}

impl<'a> MethodArg<'a> {
    pub fn as_ref<T: Atom>(&self) -> Option<&'_ T> {
        match self {
            MethodArg::Owned(d) => d.as_ref(),
            MethodArg::Ref(r) => r.as_ref(),
            MethodArg::Mut(m) => m.as_ref(),
        }
    }

    pub fn as_mut<T: Atom>(&mut self) -> Option<&'_ mut T> {
        match self {
            MethodArg::Owned(d) => d.as_mut(),
            MethodArg::Mut(m) => m.as_mut(),
            _ => None,
        }
    }

    pub fn into_owned<T: Atom>(&self) -> Option<T>
    where
        T: Clone,
    {
        match self {
            MethodArg::Owned(d) => d.as_ref().cloned(),
            MethodArg::Ref(r) => r.as_ref().cloned(),
            MethodArg::Mut(m) => m.as_ref().cloned(),
        }
    }
}

pub struct ScriptMethod {
    pub name: String,
    pub args: Vec<Entity>,
    pub ret: Entity,
    pub run: fn(Vec<MethodArg>) -> Result<Vec<Data>>,
    pub takes_self: TakesSelf,
}

impl ScriptMethod {
    pub fn run(&self, args: Vec<MethodArg<'_>>) -> Result<Vec<Data>> {
        (self.run)(args)
    }
}

pub struct ScriptVtable {
    pub methods: HashMap<String, ScriptMethod>,
}

impl ScriptVtable {
    pub fn get_method(&self, name: &str) -> Option<&ScriptMethod> {
        self.methods.get(name)
    }

    pub fn call_method(&self, name: &str, args: Vec<MethodArg<'_>>) -> Result<Vec<Data>> {
        let method = self
            .get_method(name)
            .ok_or_else(|| anyhow::anyhow!("Method {} not found", name))?;
        method.run(args)
    }
}

#[macro_export]
macro_rules! script_vtable {
    ($this:ident: $name:ty; $($method:ident => $takes_self:ident |$($access:ident $arg_names:ident: $arg_tys:ty),*| -> $ret:ty $body:block)*) => {
        fn script_vtable(&self) -> fabricate::component::ScriptVtable {
            fabricate::component::ScriptVtable {
                methods: {
                    let mut map = std::collections::HashMap::default();
                    $(
                        map.insert(stringify!($method).to_string(), fabricate::component::ScriptMethod {
                            name: stringify!($method).to_string(),
                            args: vec![$(<$arg_tys as fabricate::registry::StaticId>::static_type_uid()),*],
                            ret: <$ret as fabricate::registry::StaticId>::static_type_uid(),
                            takes_self: TakesSelf::$takes_self,
                            run: |mut args| {
                                let [$($arg_names),*] = &mut args[..] else { anyhow::bail!("Wrong number of args") };
                                $(
                                    let $arg_names = $arg_names.$access::<$arg_tys>().unwrap();
                                )*
                                let ret = $body;
                                Ok(vec![fabricate::storage::Data::new_dynamic(ret)])
                            },
                        });
                    )*
                    map
                },
            }
        }
    };
}

pub struct ValueRef<'a> {
    pub name: &'static str,
    pub typ: Entity,
    pub value: &'a mut dyn Atom,
}

pub trait Atom: Send + Sync + 'static {
    fn type_uid() -> Entity
    where
        Self: Sized,
    {
        <Self as StaticId>::static_type_uid()
    }

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn as_any_box(self: Box<Self>) -> Box<dyn Any>;

    fn clone_box(&self) -> Box<dyn Atom>;

    fn into_data(self) -> Data
    where
        Self: Sized,
    {
        Data::new_dynamic(self)
    }

    fn script_vtable(&self) -> ScriptVtable {
        ScriptVtable {
            methods: HashMap::default(),
        }
    }

    fn as_value_ref(&mut self, name: &'static str) -> ValueRef<'_>
    where
        Self: Sized,
    {
        ValueRef {
            name,
            typ: Self::type_uid(),
            value: self,
        }
    }

    fn inspect(&mut self) -> Vec<ValueRef<'_>> {
        vec![]
    }
}

macro_rules! impl_atom_simple {
    ($($t:ty),*) => {
        $(
            impl Atom for $t {
                fn as_any(&self) -> &dyn Any {
                    self
                }

                fn as_any_mut(&mut self) -> &mut dyn Any {
                    self
                }

                fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
                    self
                }

                fn clone_box(&self) -> Box<dyn Atom> {
                    Box::new(self.clone())
                }
            }
        )*
    };
}

impl_atom_simple!(
    (),
    usize,
    u8,
    u16,
    u32,
    u64,
    u128,
    isize,
    i8,
    i16,
    i32,
    i64,
    i128,
    f32,
    f64,
    bool,
    char
);

impl<T: Atom + Clone> Atom for Option<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn Atom> {
        Box::new(self.clone())
    }
}

impl<T: Atom + Clone> Atom for Vec<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn Atom> {
        Box::new(self.clone())
    }

    script_vtable!(this: Vec<T>;
        len => Ref |as_ref this: Self| -> usize { Self::len(this) }
        is_empty => Ref |as_ref this: Self| -> bool { Self::is_empty(this) }
    );
}

impl<T: Atom + Clone> Atom for std::collections::VecDeque<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn Atom> {
        Box::new(self.clone())
    }
    script_vtable!(this: std::collections::VecDeque<T>;
        len => Ref |as_ref this: Self| -> usize { Self::len(this) }
        is_empty => Ref |as_ref this: Self| -> bool { Self::is_empty(this) }
    );
}

impl<T: Send + Sync + Any> Atom for SharedLock<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn Atom> {
        Box::new(self.clone())
    }
}

impl Atom for String {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn Atom> {
        Box::new(self.clone())
    }
    script_vtable!(this: String;
        len => Ref |as_ref this: Self| -> usize { Self::len(this) }
        is_empty => Ref |as_ref this: Self| -> bool { Self::is_empty(this) }
    );
}

#[cfg(feature = "glam")]
impl Atom for glam::Vec2 {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn Atom> {
        Box::new(*self)
    }
    script_vtable!(this: glam::Vec2;
        x => Ref |as_ref this: Self| -> f32 { this.x }
        y => Ref |as_ref this: Self| -> f32 { this.y }
        length => Ref |as_ref this: Self| -> f32 { this.length() }
    );
}

#[cfg(feature = "glam")]
impl Atom for glam::Vec3 {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn Atom> {
        Box::new(*self)
    }
    script_vtable!(this: glam::Vec3;
        x => Ref |as_ref this: Self| -> f32 { this.x }
        y => Ref |as_ref this: Self| -> f32 { this.y }
        z => Ref |as_ref this: Self| -> f32 { this.z }
        length => Ref |as_ref this: Self| -> f32 { this.length() }
    );
}

#[cfg(feature = "glam")]
impl Atom for glam::Vec4 {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn Atom> {
        Box::new(*self)
    }
    script_vtable!(this: glam::Vec4;
        x => Ref |as_ref this: Self| -> f32 { this.x }
        y => Ref |as_ref this: Self| -> f32 { this.y }
        z => Ref |as_ref this: Self| -> f32 { this.z }
        w => Ref |as_ref this: Self| -> f32 { this.w }
        length => Ref |as_ref this: Self| -> f32 { this.length() }
    );
}

#[cfg(feature = "glam")]
impl Atom for glam::Quat {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn Atom> {
        Box::new(*self)
    }
    script_vtable!(this: glam::Quat;
        x => Ref |as_ref this: Self| -> f32 { this.x }
        y => Ref |as_ref this: Self| -> f32 { this.y }
        z => Ref |as_ref this: Self| -> f32 { this.z }
        w => Ref |as_ref this: Self| -> f32 { this.w }
        length => Ref |as_ref this: Self| -> f32 { this.length() }
    );
}

#[cfg(test)]
mod tests {
    use crate::world::get_world;

    #[test]
    fn test_component_primitive() {
        let world = get_world();
        let mut world = world.write();
        let e = world.spawn((0u32,)).unwrap();
        assert_eq!(
            *world
                .storage()
                .get_component::<u32>(e)
                .unwrap()
                .as_ref::<u32>()
                .unwrap(),
            0
        );
    }
}
