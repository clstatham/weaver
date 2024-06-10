use std::{any::TypeId, sync::OnceLock};

use crate::{
    registry::{TypeInfo, Typed, ValueInfo},
    Reflect,
};

pub mod glam;

macro_rules! impl_primitive {
    ($t:ty) => {
        impl Reflect for $t {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }

            fn into_any_box(self: Box<Self>) -> Box<dyn std::any::Any> {
                self
            }

            fn as_reflect(&self) -> &dyn Reflect {
                self
            }

            fn as_reflect_mut(&mut self) -> &mut dyn Reflect {
                self
            }

            fn into_reflect_box(self: Box<Self>) -> Box<dyn Reflect> {
                self
            }

            fn reflect_type_name(&self) -> &'static str {
                Self::type_name()
            }
        }

        impl Typed for $t {
            fn type_name() -> &'static str {
                stringify!($t)
            }

            fn type_info() -> &'static TypeInfo {
                static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
                TYPE_INFO.get_or_init(|| {
                    TypeInfo::Value(ValueInfo {
                        type_id: TypeId::of::<$t>(),
                        type_name: stringify!($t),
                    })
                })
            }
        }
    };
}

impl_primitive!(u8);
impl_primitive!(u16);
impl_primitive!(u32);
impl_primitive!(u64);
impl_primitive!(u128);
impl_primitive!(usize);
impl_primitive!(i8);
impl_primitive!(i16);
impl_primitive!(i32);
impl_primitive!(i64);
impl_primitive!(i128);
impl_primitive!(isize);
impl_primitive!(f32);
impl_primitive!(f64);
impl_primitive!(bool);
impl_primitive!(char);
impl_primitive!(String);
