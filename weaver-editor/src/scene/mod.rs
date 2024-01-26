use weaver::prelude::{
    weaver_ecs::component::{Data, MethodArgType, MethodWrapper},
    *,
};

pub fn register_all(registry: &std::sync::Arc<weaver_ecs::registry::Registry>) {
    LocalTransform::register_methods(registry);
    ParentInheritance::register_methods(registry);
    ParentRel::register_methods(registry);
}

#[derive(Component)]
#[method(new = "fn() -> Self")]
#[method(from_translation = "fn(Vec3) -> Self")]
#[method(from_rotation = "fn(Quat) -> Self")]
#[method(from_scale = "fn(Vec3) -> Self")]
pub struct LocalTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl LocalTransform {
    pub fn new() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Default::default()
        }
    }

    pub fn from_rotation(rotation: Quat) -> Self {
        Self {
            rotation,
            ..Default::default()
        }
    }

    pub fn from_scale(scale: Vec3) -> Self {
        Self {
            scale,
            ..Default::default()
        }
    }

    pub fn from_translation_rotation(translation: Vec3, rotation: Quat) -> Self {
        Self {
            translation,
            rotation,
            ..Default::default()
        }
    }

    pub fn from_translation_scale(translation: Vec3, scale: Vec3) -> Self {
        Self {
            translation,
            scale,
            ..Default::default()
        }
    }
}

impl Default for LocalTransform {
    fn default() -> Self {
        Self::new()
    }
}

bitflags::bitflags! {
    pub struct ParentInheritance: u32 {
        const POSITION = 1 << 0;
        const ROTATION = 1 << 1;
        const SCALE = 1 << 2;
    }
}

impl ParentInheritance {
    pub fn position() -> Self {
        Self::POSITION
    }

    pub fn rotation() -> Self {
        Self::ROTATION
    }

    pub fn scale() -> Self {
        Self::SCALE
    }
}

impl Component for ParentInheritance {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "ParentInheritance"
    }

    fn register_methods(registry: &std::sync::Arc<weaver_ecs::registry::Registry>)
    where
        Self: Sized,
    {
        let mut methods = vec![];

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "position",
            vec![],
            Some(MethodArgType::Owned(registry.get_static::<Self>())),
            move |_: &[&Data]| Ok(Self::position().into_data(None, &registry_clone)),
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "rotation",
            vec![],
            Some(MethodArgType::Owned(registry.get_static::<Self>())),
            move |_: &[&Data]| Ok(Self::rotation().into_data(None, &registry_clone)),
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "scale",
            vec![],
            Some(MethodArgType::Owned(registry.get_static::<Self>())),
            move |_: &[&Data]| Ok(Self::scale().into_data(None, &registry_clone)),
        ));

        registry.register_methods(registry.get_static::<Self>(), methods);
    }
}

#[derive(Component)]
#[method(new = "fn(&Entity, &ParentInheritance) -> Self")]
#[method(none = "fn(&Entity) -> Self")]
#[method(position = "fn(&Entity) -> Self")]
#[method(rotation = "fn(&Entity) -> Self")]
#[method(scale = "fn(&Entity) -> Self")]
#[method(position_rotation = "fn(Entity) -> Self")]
#[method(position_scale = "fn(Entity) -> Self")]
#[method(rotation_scale = "fn(Entity) -> Self")]
#[method(position_rotation_scale = "fn(Entity) -> Self")]
pub struct ParentRel {
    pub entity: Entity,
    pub inheritance: u32,
}

impl ParentRel {
    pub fn new(entity: &Entity, inheritance: &ParentInheritance) -> Self {
        Self {
            entity: *entity,
            inheritance: inheritance.bits(),
        }
    }

    pub fn none(entity: &Entity) -> Self {
        Self {
            entity: *entity,
            inheritance: ParentInheritance::empty().bits(),
        }
    }

    pub fn position(entity: &Entity) -> Self {
        Self {
            entity: *entity,
            inheritance: ParentInheritance::POSITION.bits(),
        }
    }

    pub fn rotation(entity: &Entity) -> Self {
        Self {
            entity: *entity,
            inheritance: ParentInheritance::ROTATION.bits(),
        }
    }

    pub fn scale(entity: &Entity) -> Self {
        Self {
            entity: *entity,
            inheritance: ParentInheritance::SCALE.bits(),
        }
    }

    pub fn position_rotation(entity: Entity) -> Self {
        Self {
            entity,
            inheritance: ParentInheritance::POSITION.bits() | ParentInheritance::ROTATION.bits(),
        }
    }

    pub fn position_scale(entity: Entity) -> Self {
        Self {
            entity,
            inheritance: ParentInheritance::POSITION.bits() | ParentInheritance::SCALE.bits(),
        }
    }

    pub fn rotation_scale(entity: Entity) -> Self {
        Self {
            entity,
            inheritance: ParentInheritance::ROTATION.bits() | ParentInheritance::SCALE.bits(),
        }
    }

    pub fn position_rotation_scale(entity: Entity) -> Self {
        Self {
            entity,
            inheritance: ParentInheritance::POSITION.bits()
                | ParentInheritance::ROTATION.bits()
                | ParentInheritance::SCALE.bits(),
        }
    }
}

#[system(UpdateParentRel)]
pub fn update_parent_rel(
    mut children: Query<(Entity, &ParentRel, &LocalTransform)>,
    mut global_transforms: Query<&mut Transform>,
) {
    for (child, parent_rel, local_transform) in children.iter() {
        let parent_global_transform = global_transforms.get(parent_rel.entity).unwrap();
        let mut child_global_transform = global_transforms.get(child).unwrap();

        if parent_rel.inheritance & ParentInheritance::POSITION.bits() != 0 {
            child_global_transform.set_translation(
                parent_global_transform.get_translation()
                    + parent_global_transform.get_rotation() * local_transform.translation,
            );
        }

        if parent_rel.inheritance & ParentInheritance::ROTATION.bits() != 0 {
            child_global_transform
                .set_rotation(parent_global_transform.get_rotation() * local_transform.rotation);
        }

        if parent_rel.inheritance & ParentInheritance::SCALE.bits() != 0 {
            child_global_transform
                .set_scale(parent_global_transform.get_scale() * local_transform.scale);
        }
    }
}
