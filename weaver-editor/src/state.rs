use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
};

use weaver::{
    ecs::{component::Data, registry::DynamicId},
    prelude::*,
};

pub trait EditorAction: Send + Sync + Debug {
    fn execute(&mut self, state: &mut EditorState, world: &World) -> anyhow::Result<()>;
    fn undo(&mut self, state: &mut EditorState, world: &World) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct RenameEntity {
    pub(crate) entity: Entity,
    pub(crate) old_name: Option<String>,
    pub(crate) new_name: String,
}
impl RenameEntity {
    pub fn new(entity: Entity, new_name: String) -> Self {
        Self {
            entity,
            old_name: None,
            new_name,
        }
    }
}

impl EditorAction for RenameEntity {
    fn execute(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        self.old_name = state.entity_names.get(&self.entity).cloned();

        state
            .entity_names
            .insert(self.entity, self.new_name.clone());
        Ok(())
    }

    fn undo(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        if let Some(old_name) = &self.old_name {
            state.entity_names.insert(self.entity, old_name.clone());
        } else {
            state.entity_names.remove(&self.entity);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SelectEntity {
    pub(crate) entity: Entity,
    pub(crate) previous_entity: Option<Entity>,
}

impl SelectEntity {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            previous_entity: None,
        }
    }
}

impl EditorAction for SelectEntity {
    fn execute(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        self.previous_entity = state.selected_entity;
        state.selected_entity = Some(self.entity);
        Ok(())
    }

    fn undo(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        state.selected_entity = self.previous_entity;
        Ok(())
    }
}

#[derive(Debug)]
pub struct SelectComponent {
    pub(crate) component: DynamicId,
    pub(crate) previous_component: Option<DynamicId>,
}

impl SelectComponent {
    pub fn new(component: DynamicId) -> Self {
        Self {
            component,
            previous_component: None,
        }
    }
}

impl EditorAction for SelectComponent {
    fn execute(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        self.previous_component = state.selected_component;
        state.selected_component = Some(self.component);
        Ok(())
    }

    fn undo(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        state.selected_component = self.previous_component;
        Ok(())
    }
}

#[derive(Component)]
pub struct EditorState {
    selected_entity: Option<Entity>,
    entity_names: HashMap<Entity, String>,
    selected_component: Option<DynamicId>,

    action_history: Vec<Box<dyn EditorAction>>,
    action_queue: VecDeque<Box<dyn EditorAction>>,
    undo_history: Vec<Box<dyn EditorAction>>,

    show_rename_entity: Option<Entity>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            selected_entity: None,
            selected_component: None,
            entity_names: HashMap::new(),
            show_rename_entity: None,
            action_history: Vec::new(),
            action_queue: VecDeque::new(),
            undo_history: Vec::new(),
        }
    }

    pub fn push_action(&mut self, action: Box<dyn EditorAction>) {
        self.action_queue.push_back(action);
    }

    pub fn undo(&mut self, world: &World) -> anyhow::Result<()> {
        if let Some(mut action) = self.action_history.pop() {
            log::debug!("Undo: {:?}", action);
            action.undo(self, world)?;
            self.undo_history.push(action);
        }
        Ok(())
    }

    pub fn redo(&mut self, world: &World) -> anyhow::Result<()> {
        if let Some(mut action) = self.undo_history.pop() {
            log::debug!("Redo: {:?}", action);
            action.execute(self, world)?;
            self.action_history.push(action);
        }
        Ok(())
    }

    pub fn execute_action(&mut self, world: &World) -> anyhow::Result<bool> {
        let mut executed = false;
        if let Some(mut action) = self.action_queue.pop_front() {
            log::debug!("Action: {:?}", action);
            action.execute(self, world)?;
            self.action_history.push(action);
            executed = true;
        }
        Ok(executed)
    }

    pub fn selected_entity(&self) -> Option<Entity> {
        self.selected_entity
    }

    pub fn selected_component(&self) -> Option<DynamicId> {
        self.selected_component
    }

    pub fn entity_name(&self, entity: Entity) -> Option<&String> {
        self.entity_names.get(&entity)
    }

    pub fn show_rename_entity(&mut self, entity: Entity) {
        self.show_rename_entity = Some(entity);
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EditorActions;

impl System for EditorActions {
    fn run(&self, world: std::sync::Arc<RwLock<World>>, _input: &[&Data]) -> anyhow::Result<()> {
        let world = world.read();
        let mut state = world.write_resource::<EditorState>()?;
        let input = world.read_resource::<Input>()?;
        if input.key_just_pressed(KeyCode::KeyZ) && input.key_pressed(KeyCode::ControlLeft) {
            state.undo(&world)?;
        }
        if input.key_just_pressed(KeyCode::KeyY) && input.key_pressed(KeyCode::ControlLeft) {
            state.redo(&world)?;
        }
        while state.execute_action(&world)? {}
        Ok(())
    }

    fn components_read(&self, _registry: &weaver_ecs::registry::Registry) -> Vec<DynamicId> {
        vec![]
    }

    fn components_written(&self, _registry: &weaver_ecs::registry::Registry) -> Vec<DynamicId> {
        vec![]
    }

    fn resources_read(&self, _registry: &weaver_ecs::registry::Registry) -> Vec<DynamicId> {
        vec![]
    }

    fn resources_written(&self, registry: &weaver_ecs::registry::Registry) -> Vec<DynamicId> {
        vec![registry.get_static::<EditorState>()]
    }

    fn is_exclusive(&self) -> bool {
        true
    }
}

#[system(EditorStateUi())]
pub fn editor_state_ui(mut state: ResMut<EditorState>, ctx: Res<EguiContext>) {
    ctx.draw_if_ready(|ui| {
        if let Some(entity) = state.show_rename_entity {
            let mut old_name = state.entity_names.get(&entity).cloned();
            egui::Window::new("Rename Entity")
                .default_pos(ui.available_rect().center())
                .collapsible(false)
                .resizable(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        let mut new_name = old_name.clone().unwrap_or_default();
                        let response = ui.text_edit_singleline(&mut new_name);
                        if response.lost_focus() {
                            state.show_rename_entity = None;
                        }
                        if response.changed() {
                            state.push_action(Box::new(RenameEntity {
                                entity,
                                old_name,
                                new_name,
                            }));
                        }
                    });
                });
        }
    });
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
