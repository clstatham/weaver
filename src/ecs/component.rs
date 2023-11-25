use crate::renderer::mesh::Mesh;

use super::entity::Entity;
use rustc_hash::FxHashMap;

/// Fields in a [Component].
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
    String(String),
    Vec3(glam::Vec3),
    Vec4(glam::Vec4),
    Mat3(glam::Mat3),
    Mat4(glam::Mat4),
    List(Vec<Field>),
    Mesh(Mesh),
}

/// A collection of fields that describe an [Entity].
#[derive(Debug)]
pub struct Component {
    name: String,
    pub(crate) entity: Entity,
    pub(crate) fields: FxHashMap<String, Field>,
}

impl Component {
    /// Creates a new [Component] with the given name. The [Component] will not be attached to any [Entity].
    pub fn new(name: String) -> Self {
        Component {
            name,
            entity: Entity::PLACEHOLDER,
            fields: FxHashMap::default(),
        }
    }

    /// Returns the name of the [Component].
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the [Entity] that the [Component] is attached to.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Adds a [Field] to the [Component].
    pub fn add_field(&mut self, name: &str, field: Field) {
        self.fields.insert(name.to_string(), field);
    }

    /// Returns the number of [Field]s in the [Component].
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Returns a reference to a [Field] in the [Component], or [None] if the field does not exist.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.get(name)
    }

    /// Returns a mutable reference to a [Field] in the [Component], or [None] if the field does not exist.
    pub fn field_mut(&mut self, name: &str) -> Option<&mut Field> {
        self.fields.get_mut(name)
    }
}
