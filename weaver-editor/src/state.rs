use weaver::{ecs::registry::DynamicId, prelude::*};

#[derive(Component)]
pub struct EditorState {
    pub selected_entity: Option<Entity>,
    pub selected_component: Option<DynamicId>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            selected_entity: None,
            selected_component: None,
        }
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

#[system(SelectedEntityDoodads())]
pub fn selected_entity_doodads(
    state: Res<EditorState>,
    mut doodads: ResMut<Doodads>,
    transforms: Query<&GlobalTransform>,
    meshes: Query<&Mesh, With<GlobalTransform>>,
) {
    if let Some(entity) = state.selected_entity {
        if let Some(transform) = transforms.get(entity) {
            let position = transform.get_translation();

            if let Some(mesh) = meshes.get(entity) {
                let aabb = mesh.aabb().transformed(*transform);
                let position = aabb.center();
                let doodad = Doodad::WireCube(Cube::new(
                    position,
                    Quat::IDENTITY,
                    aabb.max - aabb.min,
                    Color::GREEN,
                ));
                doodads.push(doodad);
            } else {
                let doodad =
                    Doodad::Cube(Cube::new(position, Quat::IDENTITY, Vec3::ONE, Color::GREEN));

                doodads.push(doodad);
            }
        }
    }
}
