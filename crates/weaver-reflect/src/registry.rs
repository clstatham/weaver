use std::{
    any::TypeId,
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
};

use crate::Reflect;

pub trait Typed: Reflect {
    fn type_name() -> &'static str;
    fn type_info() -> &'static TypeInfo;
}

#[derive(Debug, Clone)]
pub enum TypeInfo {
    Struct(StructInfo),
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
    pub field_indices: HashMap<&'static str, usize>,
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
        self.fields.iter().find(|field| field.name == field_name)
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

#[derive(Default)]
pub struct TypeIdHasher {
    state: u64,
}

impl Hasher for TypeIdHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write_u128(&mut self, i: u128) {
        self.state = i as u64;
    }

    fn write_u64(&mut self, i: u64) {
        self.state = i;
    }

    fn write(&mut self, _bytes: &[u8]) {
        unimplemented!("TypeIdHasher should not be used with anything other than TypeId")
    }
}

pub type TypeIdMap<T> =
    std::collections::hash_map::HashMap<TypeId, T, BuildHasherDefault<TypeIdHasher>>;

pub struct TypeRegistry {
    types: TypeIdMap<&'static TypeInfo>,
    type_names: HashMap<&'static str, TypeId>,
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
            type_names: HashMap::new(),
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
        let type_id = TypeId::of::<T>();
        let type_name = T::type_name();
        let type_info = T::type_info();
        self.types.insert(type_id, type_info);
        self.type_names.insert(type_name, type_id);
    }

    pub fn get_type_info<T: Typed>(&self) -> Option<&'static TypeInfo> {
        self.get_type_info_by_id(TypeId::of::<T>())
    }

    pub fn get_type_info_by_id(&self, type_id: TypeId) -> Option<&'static TypeInfo> {
        self.types.get(&type_id).cloned()
    }

    pub fn get_type_info_by_name(&self, type_name: &str) -> Option<&'static TypeInfo> {
        self.type_names
            .get(type_name)
            .and_then(|type_id| self.types.get(type_id))
            .cloned()
    }
}
