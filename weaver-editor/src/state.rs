use weaver::prelude::*;

#[derive(Component)]
pub struct EditorState {
    pub selected_entity: Option<Entity>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            selected_entity: None,
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
) {
    if let Some(entity) = state.selected_entity {
        if let Some(transform) = transforms.get(entity) {
            let position = transform.get_translation();

            let doodad = Doodad::Cube(Cube::new(
                position,
                Quat::IDENTITY,
                Vec3::splat(0.3),
                Color::RED,
            ));
            doodads.push(doodad);
        }
    }
}
