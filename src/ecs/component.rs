use super::entity::Entity;
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub enum Field {
    None,
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

#[derive(Debug)]
pub struct Component {
    name: String,
    pub(crate) entity: Entity,
    pub(crate) fields: FxHashMap<String, Field>,
}

impl Component {
    pub fn new(name: String) -> Self {
        Component {
            name,
            entity: Entity::PLACEHOLDER,
            fields: FxHashMap::default(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }

    pub fn add_field(&mut self, name: &str, field: Field) {
        self.fields.insert(name.to_string(), field);
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.get(name)
    }

    pub fn field_mut(&mut self, name: &str) -> Option<&mut Field> {
        self.fields.get_mut(name)
    }
}
