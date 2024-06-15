use std::{any::TypeId, collections::HashMap, hash::Hash, sync::OnceLock};

use weaver_util::TypeIdMap;

use crate::reflect::{
    registry::{ListInfo, MapInfo, TypeInfo, TypeRegistration, Typed, ValueInfo},
    Reflect,
};

use super::registry::{List, Map};

pub mod ecs;
pub mod glam;

macro_rules! impl_primitive {
    ($t:ty) => {
        impl Reflect for $t {
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

            fn get_type_registration() -> TypeRegistration {
                TypeRegistration {
                    type_id: TypeId::of::<$t>(),
                    type_name: Self::type_name(),
                    type_info: Self::type_info(),
                    type_aux_data: TypeIdMap::default(),
                }
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

impl<T: Reflect + Typed> Reflect for Vec<T> {
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

impl<T: Reflect + Typed> Typed for Vec<T> {
    fn type_name() -> &'static str {
        static TYPE_NAME: OnceLock<String> = OnceLock::new();
        TYPE_NAME.get_or_init(|| format!("Vec<{}>", T::type_name()))
    }

    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
        TYPE_INFO.get_or_init(|| TypeInfo::List(ListInfo::new::<Vec<T>, T>()))
    }

    fn get_type_registration() -> TypeRegistration {
        TypeRegistration {
            type_id: TypeId::of::<Vec<T>>(),
            type_name: Self::type_name(),
            type_info: Self::type_info(),
            type_aux_data: TypeIdMap::default(),
        }
    }
}

impl<T: Reflect + Typed> List for Vec<T> {
    fn len_reflect(&self) -> usize {
        self.len()
    }

    fn get_reflect(&self, index: usize) -> Option<&dyn Reflect> {
        self.get(index).map(|v| v as &dyn Reflect)
    }

    fn get_mut_reflect(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.get_mut(index).map(|v| v as &mut dyn Reflect)
    }

    fn insert_reflect(&mut self, index: usize, value: Box<dyn Reflect>) {
        let Ok(value) = value.downcast::<T>() else {
            panic!("downcast failed: expected {}", T::type_name());
        };
        self.insert(index, *value);
    }

    fn remove_reflect(&mut self, index: usize) -> Option<Box<dyn Reflect>> {
        Some(Box::new(self.remove(index)))
    }

    fn clear_reflect(&mut self) {
        self.clear();
    }

    fn drain_reflect(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
        self.into_iter()
            .map(|item| Box::new(item) as Box<dyn Reflect>)
            .collect()
    }

    fn push_reflect(&mut self, value: Box<dyn Reflect>) {
        let Ok(value) = value.downcast::<T>() else {
            panic!("downcast failed: expected {}", T::type_name());
        };
        self.push(*value);
    }

    fn pop_reflect(&mut self) -> Option<Box<dyn Reflect>> {
        self.pop().map(|v| Box::new(v) as Box<dyn Reflect>)
    }
}

impl<K: Reflect + Typed, V: Reflect + Typed> Reflect for HashMap<K, V> {
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
        static TYPE_NAME: OnceLock<String> = OnceLock::new();
        TYPE_NAME.get_or_init(|| format!("HashMap<{}, {}>", K::type_name(), V::type_name()))
    }
}

impl<K: Reflect + Typed, V: Reflect + Typed> Typed for HashMap<K, V> {
    fn type_name() -> &'static str {
        static TYPE_NAME: OnceLock<String> = OnceLock::new();
        TYPE_NAME.get_or_init(|| format!("HashMap<{}, {}>", K::type_name(), V::type_name()))
    }

    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
        TYPE_INFO.get_or_init(|| TypeInfo::Map(MapInfo::new::<HashMap<K, V>, K, V>()))
    }

    fn get_type_registration() -> TypeRegistration {
        TypeRegistration {
            type_id: TypeId::of::<HashMap<K, V>>(),
            type_name: Self::type_name(),
            type_info: Self::type_info(),
            type_aux_data: TypeIdMap::default(),
        }
    }
}

impl<K: Reflect + Typed + Hash + Eq, V: Reflect + Typed> Map for HashMap<K, V> {
    fn len_reflect(&self) -> usize {
        self.len()
    }

    fn get_reflect(&self, key: &dyn Reflect) -> Option<&dyn Reflect> {
        let key = key.downcast_ref::<K>()?;
        self.get(key).map(|v| v as &dyn Reflect)
    }

    fn get_mut_reflect(&mut self, key: &dyn Reflect) -> Option<&mut dyn Reflect> {
        let key = key.downcast_ref::<K>()?;
        self.get_mut(key).map(|v| v as &mut dyn Reflect)
    }

    fn insert_reflect(&mut self, key: Box<dyn Reflect>, value: Box<dyn Reflect>) {
        let Ok(key) = key.downcast::<K>() else {
            panic!("downcast failed: expected {}", K::type_name());
        };
        let Ok(value) = value.downcast::<V>() else {
            panic!("downcast failed: expected {}", V::type_name());
        };
        self.insert(*key, *value);
    }

    fn remove_reflect(&mut self, key: &dyn Reflect) -> Option<Box<dyn Reflect>> {
        let key = key.downcast_ref::<K>()?;
        self.remove(key).map(|v| Box::new(v) as Box<dyn Reflect>)
    }

    fn clear_reflect(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflect() {
        let value = 42u8;
        let reflect = value.as_reflect();
        assert_eq!(reflect.reflect_type_name(), "u8");
        let value = reflect.downcast_ref::<u8>().unwrap();
        assert_eq!(*value, 42);
    }

    #[test]
    fn test_vec() {
        let vec = vec![1, 2, 3];
        let reflect = vec.as_reflect();
        assert_eq!(reflect.reflect_type_name(), "Vec<i32>");
        let vec = reflect.downcast_ref::<Vec<i32>>().unwrap();
        assert_eq!(vec, &[1, 2, 3]);
    }

    #[test]
    fn test_hash_map() {
        let mut map = HashMap::new();
        map.insert(1, 2);
        map.insert(3, 4);
        let reflect = map.as_reflect();
        assert_eq!(reflect.reflect_type_name(), "HashMap<i32, i32>");
        let map = reflect.downcast_ref::<HashMap<i32, i32>>().unwrap();
        assert_eq!(map.get(&1), Some(&2));
        assert_eq!(map.get(&3), Some(&4));
    }
}
