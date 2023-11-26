use crate::renderer::mesh::Mesh;

use super::entity::Entity;
use rustc_hash::FxHashMap;

/// Fields in a [Component].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    GuiElement(crate::gui::element::GuiElement),
}

impl Field {
    pub fn type_name(&self) -> &'static str {
        match self {
            Field::None => "None",
            Field::Bool(_) => "Bool",
            Field::U8(_) => "U8",
            Field::U16(_) => "U16",
            Field::U32(_) => "U32",
            Field::U64(_) => "U64",
            Field::I8(_) => "I8",
            Field::I16(_) => "I16",
            Field::I32(_) => "I32",
            Field::I64(_) => "I64",
            Field::F32(_) => "F32",
            Field::F64(_) => "F64",
            Field::String(_) => "String",
            Field::Vec3(_) => "Vec3",
            Field::Vec4(_) => "Vec4",
            Field::Mat3(_) => "Mat3",
            Field::Mat4(_) => "Mat4",
            Field::List(_) => "List",
            Field::Mesh(_) => "Mesh",
            Field::GuiElement(_) => "GuiElement",
        }
    }

    pub fn add(&self, other: &Field) -> Option<Field> {
        match (self, other) {
            (Field::U8(a), Field::U8(b)) => Some(Field::U8(a + b)),
            (Field::U16(a), Field::U16(b)) => Some(Field::U16(a + b)),
            (Field::U32(a), Field::U32(b)) => Some(Field::U32(a + b)),
            (Field::U64(a), Field::U64(b)) => Some(Field::U64(a + b)),
            (Field::I8(a), Field::I8(b)) => Some(Field::I8(a + b)),
            (Field::I16(a), Field::I16(b)) => Some(Field::I16(a + b)),
            (Field::I32(a), Field::I32(b)) => Some(Field::I32(a + b)),
            (Field::I64(a), Field::I64(b)) => Some(Field::I64(a + b)),
            (Field::F32(a), Field::F32(b)) => Some(Field::F32(a + b)),
            (Field::F64(a), Field::F64(b)) => Some(Field::F64(a + b)),
            _ => None,
        }
    }

    pub fn subtract(&self, other: &Field) -> Option<Field> {
        match (self, other) {
            (Field::U8(a), Field::U8(b)) => Some(Field::U8(a - b)),
            (Field::U16(a), Field::U16(b)) => Some(Field::U16(a - b)),
            (Field::U32(a), Field::U32(b)) => Some(Field::U32(a - b)),
            (Field::U64(a), Field::U64(b)) => Some(Field::U64(a - b)),
            (Field::I8(a), Field::I8(b)) => Some(Field::I8(a - b)),
            (Field::I16(a), Field::I16(b)) => Some(Field::I16(a - b)),
            (Field::I32(a), Field::I32(b)) => Some(Field::I32(a - b)),
            (Field::I64(a), Field::I64(b)) => Some(Field::I64(a - b)),
            (Field::F32(a), Field::F32(b)) => Some(Field::F32(a - b)),
            (Field::F64(a), Field::F64(b)) => Some(Field::F64(a - b)),
            _ => None,
        }
    }

    pub fn multiply(&self, other: &Field) -> Option<Field> {
        match (self, other) {
            (Field::U8(a), Field::U8(b)) => Some(Field::U8(a * b)),
            (Field::U16(a), Field::U16(b)) => Some(Field::U16(a * b)),
            (Field::U32(a), Field::U32(b)) => Some(Field::U32(a * b)),
            (Field::U64(a), Field::U64(b)) => Some(Field::U64(a * b)),
            (Field::I8(a), Field::I8(b)) => Some(Field::I8(a * b)),
            (Field::I16(a), Field::I16(b)) => Some(Field::I16(a * b)),
            (Field::I32(a), Field::I32(b)) => Some(Field::I32(a * b)),
            (Field::I64(a), Field::I64(b)) => Some(Field::I64(a * b)),
            (Field::F32(a), Field::F32(b)) => Some(Field::F32(a * b)),
            (Field::F64(a), Field::F64(b)) => Some(Field::F64(a * b)),
            _ => None,
        }
    }

    pub fn divide(&self, other: &Field) -> Option<Field> {
        match (self, other) {
            (Field::U8(a), Field::U8(b)) => Some(Field::U8(a / b)),
            (Field::U16(a), Field::U16(b)) => Some(Field::U16(a / b)),
            (Field::U32(a), Field::U32(b)) => Some(Field::U32(a / b)),
            (Field::U64(a), Field::U64(b)) => Some(Field::U64(a / b)),
            (Field::I8(a), Field::I8(b)) => Some(Field::I8(a / b)),
            (Field::I16(a), Field::I16(b)) => Some(Field::I16(a / b)),
            (Field::I32(a), Field::I32(b)) => Some(Field::I32(a / b)),
            (Field::I64(a), Field::I64(b)) => Some(Field::I64(a / b)),
            (Field::F32(a), Field::F32(b)) => Some(Field::F32(a / b)),
            (Field::F64(a), Field::F64(b)) => Some(Field::F64(a / b)),
            _ => None,
        }
    }

    pub fn negate(&self) -> Option<Field> {
        match self {
            Field::I8(a) => Some(Field::I8(-a)),
            Field::I16(a) => Some(Field::I16(-a)),
            Field::I32(a) => Some(Field::I32(-a)),
            Field::I64(a) => Some(Field::I64(-a)),
            Field::F32(a) => Some(Field::F32(-a)),
            Field::F64(a) => Some(Field::F64(-a)),
            Field::Vec3(a) => Some(Field::Vec3(*a * -1.0)),
            Field::Vec4(a) => Some(Field::Vec4(*a * -1.0)),
            _ => None,
        }
    }
}

/// A collection of fields that describe an [Entity].
#[derive(Debug, serde::Serialize, serde::Deserialize)]
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
