use std::{any::TypeId, sync::OnceLock};

use crate::reflect::{
    registry::{FieldInfo, Struct, StructInfo, TypeInfo, Typed},
    Reflect,
};

impl Struct for glam::Vec2 {
    fn field(&self, field_name: &str) -> Option<&dyn Reflect> {
        match field_name {
            "x" => Some(&self.x),
            "y" => Some(&self.y),
            _ => None,
        }
    }

    fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn Reflect> {
        match field_name {
            "x" => Some(&mut self.x),
            "y" => Some(&mut self.y),
            _ => None,
        }
    }
}

impl Typed for glam::Vec2 {
    fn type_name() -> &'static str {
        "glam::Vec2"
    }

    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
        TYPE_INFO.get_or_init(|| {
            TypeInfo::Struct(StructInfo {
                type_id: TypeId::of::<glam::Vec2>(),
                type_name: "glam::Vec2",
                fields: vec![
                    FieldInfo {
                        name: "x",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                    FieldInfo {
                        name: "y",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                ]
                .into(),
                field_names: vec!["x", "y"].into(),
                field_indices: vec![("x", 0), ("y", 1)].into_iter().collect(),
            })
        })
    }
}

impl Struct for glam::Vec3 {
    fn field(&self, field_name: &str) -> Option<&dyn Reflect> {
        match field_name {
            "x" => Some(&self.x),
            "y" => Some(&self.y),
            "z" => Some(&self.z),
            _ => None,
        }
    }

    fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn Reflect> {
        match field_name {
            "x" => Some(&mut self.x),
            "y" => Some(&mut self.y),
            "z" => Some(&mut self.z),
            _ => None,
        }
    }
}

impl Typed for glam::Vec3 {
    fn type_name() -> &'static str {
        "glam::Vec3"
    }

    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
        TYPE_INFO.get_or_init(|| {
            TypeInfo::Struct(StructInfo {
                type_id: TypeId::of::<glam::Vec3>(),
                type_name: "glam::Vec3",
                fields: vec![
                    FieldInfo {
                        name: "x",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                    FieldInfo {
                        name: "y",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                    FieldInfo {
                        name: "z",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                ]
                .into(),
                field_names: vec!["x", "y", "z"].into(),
                field_indices: vec![("x", 0), ("y", 1), ("z", 2)].into_iter().collect(),
            })
        })
    }
}

impl Struct for glam::Vec4 {
    fn field(&self, field_name: &str) -> Option<&dyn Reflect> {
        match field_name {
            "x" => Some(&self.x),
            "y" => Some(&self.y),
            "z" => Some(&self.z),
            "w" => Some(&self.w),
            _ => None,
        }
    }

    fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn Reflect> {
        match field_name {
            "x" => Some(&mut self.x),
            "y" => Some(&mut self.y),
            "z" => Some(&mut self.z),
            "w" => Some(&mut self.w),
            _ => None,
        }
    }
}

impl Typed for glam::Vec4 {
    fn type_name() -> &'static str {
        "glam::Vec4"
    }

    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
        TYPE_INFO.get_or_init(|| {
            TypeInfo::Struct(StructInfo {
                type_id: TypeId::of::<glam::Vec4>(),
                type_name: "glam::Vec4",
                fields: vec![
                    FieldInfo {
                        name: "x",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                    FieldInfo {
                        name: "y",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                    FieldInfo {
                        name: "z",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                    FieldInfo {
                        name: "w",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                ]
                .into(),
                field_names: vec!["x", "y", "z", "w"].into(),
                field_indices: vec![("x", 0), ("y", 1), ("z", 2), ("w", 3)]
                    .into_iter()
                    .collect(),
            })
        })
    }
}

impl Struct for glam::Mat2 {
    fn field(&self, field_name: &str) -> Option<&dyn Reflect> {
        match field_name {
            "x_axis" => Some(&self.x_axis),
            "y_axis" => Some(&self.y_axis),
            _ => None,
        }
    }

    fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn Reflect> {
        match field_name {
            "x_axis" => Some(&mut self.x_axis),
            "y_axis" => Some(&mut self.y_axis),
            _ => None,
        }
    }
}

impl Typed for glam::Mat2 {
    fn type_name() -> &'static str {
        "glam::Mat2"
    }

    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
        TYPE_INFO.get_or_init(|| {
            TypeInfo::Struct(StructInfo {
                type_id: TypeId::of::<glam::Mat2>(),
                type_name: "glam::Mat2",
                fields: vec![
                    FieldInfo {
                        name: "x_axis",
                        type_id: TypeId::of::<glam::Vec2>(),
                        type_name: "glam::Vec2",
                    },
                    FieldInfo {
                        name: "y_axis",
                        type_id: TypeId::of::<glam::Vec2>(),
                        type_name: "glam::Vec2",
                    },
                ]
                .into(),
                field_names: vec!["x_axis", "y_axis"].into(),
                field_indices: vec![("x_axis", 0), ("y_axis", 1)].into_iter().collect(),
            })
        })
    }
}

impl Struct for glam::Mat3 {
    fn field(&self, field_name: &str) -> Option<&dyn Reflect> {
        match field_name {
            "x_axis" => Some(&self.x_axis),
            "y_axis" => Some(&self.y_axis),
            "z_axis" => Some(&self.z_axis),
            _ => None,
        }
    }

    fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn Reflect> {
        match field_name {
            "x_axis" => Some(&mut self.x_axis),
            "y_axis" => Some(&mut self.y_axis),
            "z_axis" => Some(&mut self.z_axis),
            _ => None,
        }
    }
}

impl Typed for glam::Mat3 {
    fn type_name() -> &'static str {
        "glam::Mat3"
    }

    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
        TYPE_INFO.get_or_init(|| {
            TypeInfo::Struct(StructInfo {
                type_id: TypeId::of::<glam::Mat3>(),
                type_name: "glam::Mat3",
                fields: vec![
                    FieldInfo {
                        name: "x_axis",
                        type_id: TypeId::of::<glam::Vec3>(),
                        type_name: "glam::Vec3",
                    },
                    FieldInfo {
                        name: "y_axis",
                        type_id: TypeId::of::<glam::Vec3>(),
                        type_name: "glam::Vec3",
                    },
                    FieldInfo {
                        name: "z_axis",
                        type_id: TypeId::of::<glam::Vec3>(),
                        type_name: "glam::Vec3",
                    },
                ]
                .into(),
                field_names: vec!["x_axis", "y_axis", "z_axis"].into(),
                field_indices: vec![("x_axis", 0), ("y_axis", 1), ("z_axis", 2)]
                    .into_iter()
                    .collect(),
            })
        })
    }
}

impl Struct for glam::Mat4 {
    fn field(&self, field_name: &str) -> Option<&dyn Reflect> {
        match field_name {
            "x_axis" => Some(&self.x_axis),
            "y_axis" => Some(&self.y_axis),
            "z_axis" => Some(&self.z_axis),
            "w_axis" => Some(&self.w_axis),
            _ => None,
        }
    }

    fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn Reflect> {
        match field_name {
            "x_axis" => Some(&mut self.x_axis),
            "y_axis" => Some(&mut self.y_axis),
            "z_axis" => Some(&mut self.z_axis),
            "w_axis" => Some(&mut self.w_axis),
            _ => None,
        }
    }
}

impl Typed for glam::Mat4 {
    fn type_name() -> &'static str {
        "glam::Mat4"
    }

    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
        TYPE_INFO.get_or_init(|| {
            TypeInfo::Struct(StructInfo {
                type_id: TypeId::of::<glam::Mat4>(),
                type_name: "glam::Mat4",
                fields: vec![
                    FieldInfo {
                        name: "x_axis",
                        type_id: TypeId::of::<glam::Vec4>(),
                        type_name: "glam::Vec4",
                    },
                    FieldInfo {
                        name: "y_axis",
                        type_id: TypeId::of::<glam::Vec4>(),
                        type_name: "glam::Vec4",
                    },
                    FieldInfo {
                        name: "z_axis",
                        type_id: TypeId::of::<glam::Vec4>(),
                        type_name: "glam::Vec4",
                    },
                    FieldInfo {
                        name: "w_axis",
                        type_id: TypeId::of::<glam::Vec4>(),
                        type_name: "glam::Vec4",
                    },
                ]
                .into(),
                field_names: vec!["x_axis", "y_axis", "z_axis", "w_axis"].into(),
                field_indices: vec![("x_axis", 0), ("y_axis", 1), ("z_axis", 2), ("w_axis", 3)]
                    .into_iter()
                    .collect(),
            })
        })
    }
}

impl Struct for glam::Quat {
    fn field(&self, field_name: &str) -> Option<&dyn Reflect> {
        match field_name {
            "x" => Some(&self.x),
            "y" => Some(&self.y),
            "z" => Some(&self.z),
            "w" => Some(&self.w),
            _ => None,
        }
    }

    fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn Reflect> {
        match field_name {
            "x" => Some(&mut self.x),
            "y" => Some(&mut self.y),
            "z" => Some(&mut self.z),
            "w" => Some(&mut self.w),
            _ => None,
        }
    }
}

impl Typed for glam::Quat {
    fn type_name() -> &'static str {
        "glam::Quat"
    }

    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
        TYPE_INFO.get_or_init(|| {
            TypeInfo::Struct(StructInfo {
                type_id: TypeId::of::<glam::Quat>(),
                type_name: "glam::Quat",
                fields: vec![
                    FieldInfo {
                        name: "x",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                    FieldInfo {
                        name: "y",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                    FieldInfo {
                        name: "z",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                    FieldInfo {
                        name: "w",
                        type_id: TypeId::of::<f32>(),
                        type_name: "f32",
                    },
                ]
                .into(),
                field_names: vec!["x", "y", "z", "w"].into(),
                field_indices: vec![("x", 0), ("y", 1), ("z", 2), ("w", 3)]
                    .into_iter()
                    .collect(),
            })
        })
    }
}
