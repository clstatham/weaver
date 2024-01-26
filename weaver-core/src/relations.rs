use weaver_ecs::prelude::*;

use crate::prelude::{GlobalTransform, Transform};

#[system(UpdateTransforms())]
pub fn update_transforms(
    graph: Res<EntityGraph>,
    mut transforms: Query<(Entity, &mut GlobalTransform, &Transform)>,
) {
    for (entity, mut global_transform, transform) in transforms.iter() {
        if let Some(parent) = graph.get_parent(entity) {
            if let Some((_, parent_global_transform, _)) = transforms.get(parent) {
                global_transform.set_translation(
                    parent_global_transform.get_translation()
                        + parent_global_transform.get_rotation() * transform.translation,
                );
                global_transform
                    .set_rotation(parent_global_transform.get_rotation() * transform.rotation);
                global_transform.set_scale(parent_global_transform.get_scale() * transform.scale);
            }
        } else {
            global_transform.set_translation(transform.translation);
            global_transform.set_rotation(transform.rotation);
            global_transform.set_scale(transform.scale);
        }
    }
}
