use std::{any::TypeId, sync::Arc};

use crate::prelude::Resource;
use crate::{self as weaver_ecs, prelude::Component};
use weaver_util::{impl_downcast, DowncastSync, FxHashMap};

pub use weaver_util::TypeIdMap;

use super::Reflect;

pub trait Typed: 'static {
    fn type_name() -> &'static str
    where
        Self: Sized;
    fn type_info() -> &'static TypeInfo
    where
        Self: Sized;
    fn get_type_registration() -> TypeRegistration
    where
        Self: Sized,
    {
        let type_info = Self::type_info();
        TypeRegistration {
            type_id: TypeId::of::<Self>(),
            type_name: Self::type_name(),
            type_info,
            type_aux_data: TypeIdMap::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TypeInfo {
    Struct(StructInfo),
    List(ListInfo),
    Map(MapInfo),
    Value(ValueInfo),
}

#[derive(Debug, Clone)]
pub struct ValueInfo {
    pub type_id: TypeId,
    pub type_name: &'static str,
}

pub trait Struct: Reflect {
    fn field(&self, field_name: &str) -> Option<&dyn Reflect>;
    fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn Reflect>;
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub fields: Box<[FieldInfo]>,
    pub field_names: Box<[&'static str]>,
    pub field_indices: FxHashMap<&'static str, usize>,
}

impl StructInfo {
    pub fn new<T: Reflect + Typed>(fields: &[FieldInfo]) -> Self {
        let type_id = TypeId::of::<T>();
        let type_name = T::type_name();
        let field_names: Box<[&'static str]> = fields.iter().map(|field| field.name).collect();
        let field_indices = field_names
            .iter()
            .enumerate()
            .map(|(i, name)| (*name, i))
            .collect();
        Self {
            type_id,
            type_name,
            fields: fields.into(),
            field_names,
            field_indices,
        }
    }

    pub fn field(&self, field_name: &str) -> Option<&FieldInfo> {
        self.field_index(field_name)
            .map(|index| &self.fields[index])
    }

    pub fn field_index(&self, field_name: &str) -> Option<usize> {
        self.field_indices.get(field_name).copied()
    }

    pub fn is<T: Reflect + Typed>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: &'static str,
    pub type_name: &'static str,
    pub type_id: TypeId,
}

pub trait List: Reflect {
    fn len_reflect(&self) -> usize;
    fn is_empty_reflect(&self) -> bool {
        self.len_reflect() == 0
    }
    fn get_reflect(&self, index: usize) -> Option<&dyn Reflect>;
    fn get_mut_reflect(&mut self, index: usize) -> Option<&mut dyn Reflect>;
    fn insert_reflect(&mut self, index: usize, value: Box<dyn Reflect>);
    fn push_reflect(&mut self, value: Box<dyn Reflect>) {
        self.insert_reflect(self.len_reflect(), value);
    }
    fn remove_reflect(&mut self, index: usize) -> Option<Box<dyn Reflect>>;
    fn clear_reflect(&mut self);
    fn pop_reflect(&mut self) -> Option<Box<dyn Reflect>> {
        if self.is_empty_reflect() {
            None
        } else {
            self.remove_reflect(self.len_reflect() - 1)
        }
    }
    fn drain_reflect(self: Box<Self>) -> Vec<Box<dyn Reflect>>;
}

#[derive(Debug, Clone)]
pub struct ListInfo {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub item_type_id: TypeId,
    pub item_type_name: &'static str,
}

impl ListInfo {
    pub fn new<L: Reflect + Typed, I: Reflect + Typed>() -> Self {
        Self {
            type_id: TypeId::of::<L>(),
            type_name: L::type_name(),
            item_type_id: TypeId::of::<I>(),
            item_type_name: I::type_name(),
        }
    }

    pub fn is<L: Reflect + Typed>(&self) -> bool {
        self.type_id == TypeId::of::<L>()
    }

    pub fn item_is<I: Reflect + Typed>(&self) -> bool {
        self.item_type_id == TypeId::of::<I>()
    }
}

pub trait Map: Reflect {
    fn len_reflect(&self) -> usize;
    fn is_empty_reflect(&self) -> bool {
        self.len_reflect() == 0
    }
    fn get_reflect(&self, key: &dyn Reflect) -> Option<&dyn Reflect>;
    fn get_mut_reflect(&mut self, key: &dyn Reflect) -> Option<&mut dyn Reflect>;
    fn insert_reflect(&mut self, key: Box<dyn Reflect>, value: Box<dyn Reflect>);
    fn remove_reflect(&mut self, key: &dyn Reflect) -> Option<Box<dyn Reflect>>;
    fn clear_reflect(&mut self);
}

#[derive(Debug, Clone)]
pub struct MapInfo {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub key_type_id: TypeId,
    pub key_type_name: &'static str,
    pub value_type_id: TypeId,
    pub value_type_name: &'static str,
}

impl MapInfo {
    pub fn new<M: Reflect + Typed, K: Reflect + Typed, V: Reflect + Typed>() -> Self {
        Self {
            type_id: TypeId::of::<M>(),
            type_name: M::type_name(),
            key_type_id: TypeId::of::<K>(),
            key_type_name: K::type_name(),
            value_type_id: TypeId::of::<V>(),
            value_type_name: V::type_name(),
        }
    }

    pub fn is<M: Reflect + Typed>(&self) -> bool {
        self.type_id == TypeId::of::<M>()
    }

    pub fn key_is<K: Reflect + Typed>(&self) -> bool {
        self.key_type_id == TypeId::of::<K>()
    }

    pub fn value_is<V: Reflect + Typed>(&self) -> bool {
        self.value_type_id == TypeId::of::<V>()
    }
}

pub trait TypeAuxData: DowncastSync {
    fn clone_type_aux_data(&self) -> Arc<dyn TypeAuxData>;
}
impl_downcast!(TypeAuxData);

impl<T: Clone + DowncastSync> TypeAuxData for T {
    fn clone_type_aux_data(&self) -> Arc<dyn TypeAuxData> {
        Arc::new(self.clone())
    }
}

pub trait FromType<T: Sized> {
    fn from_type() -> Self;
}

pub trait FromReflect: Sized {
    fn from_reflect(reflect: &dyn Reflect) -> Option<&Self>;
    fn from_reflect_mut(reflect: &mut dyn Reflect) -> Option<&mut Self>;
}

pub trait FromComponent {
    fn from_component(component: &dyn Component) -> Option<&dyn Reflect>;
    fn from_component_mut(component: &mut dyn Component) -> Option<&mut dyn Reflect>;
}

pub struct TypeRegistration {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub type_info: &'static TypeInfo,
    pub type_aux_data: TypeIdMap<Arc<dyn TypeAuxData>>,
}

#[derive(Resource)]
pub struct TypeRegistry {
    types: TypeIdMap<TypeRegistration>,
    type_names: FxHashMap<&'static str, TypeId>,
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::empty()
    }
}

impl TypeRegistry {
    pub fn empty() -> Self {
        Self {
            types: TypeIdMap::default(),
            type_names: FxHashMap::default(),
        }
    }

    pub fn new() -> Self {
        let mut registry = Self::empty();
        registry.register::<u8>();
        registry.register::<u16>();
        registry.register::<u32>();
        registry.register::<u64>();
        registry.register::<u128>();
        registry.register::<usize>();
        registry.register::<i8>();
        registry.register::<i16>();
        registry.register::<i32>();
        registry.register::<i64>();
        registry.register::<i128>();
        registry.register::<isize>();
        registry.register::<f32>();
        registry.register::<f64>();
        registry.register::<bool>();
        registry.register::<String>();
        registry
    }

    pub fn register<T: Typed>(&mut self) {
        if self.types.contains_key(&TypeId::of::<T>()) {
            return;
        }
        let type_registration = T::get_type_registration();
        self.types.insert(TypeId::of::<T>(), type_registration);
    }

    pub fn get_type_info<T: Reflect>(&self) -> Option<&TypeRegistration> {
        self.get_type_info_by_id(TypeId::of::<T>())
    }

    pub fn get_type_info_by_id(&self, type_id: TypeId) -> Option<&TypeRegistration> {
        self.types.get(&type_id)
    }

    pub fn get_type_info_by_name(&self, type_name: &str) -> Option<&TypeRegistration> {
        self.type_names
            .get(type_name)
            .and_then(|type_id| self.types.get(type_id))
    }

    pub fn get_type_data<T: Reflect, D: TypeAuxData>(&self) -> Option<&D> {
        self.get_type_data_by_id(TypeId::of::<T>())
    }

    pub fn get_type_data_by_id<D: TypeAuxData>(&self, type_id: TypeId) -> Option<&D> {
        self.types.get(&type_id).and_then(|type_registration| {
            type_registration
                .type_aux_data
                .get(&type_id)
                .and_then(|type_aux_data| type_aux_data.downcast_ref())
        })
    }

    pub fn register_type_data<T: Reflect, D: TypeAuxData + FromType<T>>(&mut self) {
        let type_id = TypeId::of::<T>();
        let type_registration = self.types.get_mut(&type_id).unwrap();
        type_registration
            .type_aux_data
            .insert(type_id, Arc::new(D::from_type()));
    }
}
