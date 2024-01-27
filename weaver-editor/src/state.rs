use std::{any::TypeId, collections::HashMap, fmt::Debug, sync::Arc};

use weaver::{
    ecs::{
        component::{Data, Downcast},
        registry::DynamicId,
    },
    prelude::*,
};

pub trait EditorAction: Send + Sync + Debug + Downcast + 'static {
    #[allow(unused_variables)]
    fn begin(&mut self, state: &mut EditorState, world: &World) -> anyhow::Result<()> {
        Ok(())
    }
    #[allow(unused_variables)]
    fn update(&mut self, state: &mut EditorState, world: &World) -> anyhow::Result<()> {
        Ok(())
    }
    #[allow(unused_variables)]
    fn end(&mut self, state: &mut EditorState, world: &World) -> anyhow::Result<()> {
        Ok(())
    }
    fn undo(&mut self, state: &mut EditorState, world: &World) -> anyhow::Result<()>;
    fn redo(&mut self, state: &mut EditorState, world: &World) -> anyhow::Result<()> {
        self.begin(state, world)?;
        self.end(state, world)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct RenameEntity {
    pub(crate) entity: Entity,
    pub(crate) old_name: Option<String>,
}
impl RenameEntity {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            old_name: None,
        }
    }
}

impl EditorAction for RenameEntity {
    fn begin(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        self.old_name = state.entity_names.get(&self.entity).cloned();
        Ok(())
    }

    fn end(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        let new_name = std::mem::take(&mut state.entity_rename_buffer);
        state.entity_names.insert(self.entity, new_name);
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
    pub(crate) previous_component: Option<DynamicId>,
}

impl SelectEntity {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            previous_entity: None,
            previous_component: None,
        }
    }
}

impl EditorAction for SelectEntity {
    fn begin(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        self.previous_entity = state.selected_entity;
        self.previous_component = state.selected_component;
        Ok(())
    }

    fn end(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        state.selected_component = None;
        state.selected_entity = Some(self.entity);
        Ok(())
    }

    fn undo(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        state.selected_component = self.previous_component;
        state.selected_entity = self.previous_entity;
        Ok(())
    }
}

#[derive(Debug)]
pub struct UpdateComponent {
    pub(crate) entity: Entity,
    pub(crate) component: DynamicId,
    pub(crate) previous_fields: Option<Vec<Data>>,
    pub(crate) new_fields: Option<Vec<Data>>,
}

impl UpdateComponent {
    pub fn new(entity: Entity, component: DynamicId) -> Self {
        Self {
            entity,
            component,
            previous_fields: None,
            new_fields: None,
        }
    }
}

impl EditorAction for UpdateComponent {
    fn begin(&mut self, _state: &mut EditorState, world: &World) -> anyhow::Result<()> {
        let query = world.query_dynamic().read_id(self.component).build();
        let data = query.get(self.entity).ok_or(anyhow::anyhow!(
            "Entity {:?} does not have component {:?}",
            self.entity,
            self.component
        ))?;
        let data = &data[0];
        self.previous_fields = data.data().fields();
        Ok(())
    }

    fn end(&mut self, _state: &mut EditorState, world: &World) -> anyhow::Result<()> {
        let query = world.query_dynamic().read_id(self.component).build();
        let data = query.get(self.entity).ok_or(anyhow::anyhow!(
            "Entity {:?} does not have component {:?}",
            self.entity,
            self.component
        ))?;
        let data = &data[0];
        self.new_fields = data.data().fields();
        Ok(())
    }

    fn undo(&mut self, _state: &mut EditorState, world: &World) -> anyhow::Result<()> {
        let query = world.query_dynamic().write_id(self.component).build();
        let mut data = query.get(self.entity).ok_or(anyhow::anyhow!(
            "Entity {:?} does not have component {:?}",
            self.entity,
            self.component
        ))?;
        let data = &mut data[0];
        if let Some(fields) = &self.previous_fields {
            for field in fields {
                data.data_mut()
                    .unwrap()
                    .set_field_by_name(field.field_name().unwrap(), field.to_owned())?;
            }
        }

        Ok(())
    }

    fn redo(&mut self, _state: &mut EditorState, world: &World) -> anyhow::Result<()> {
        let query = world.query_dynamic().write_id(self.component).build();
        let mut data = query.get(self.entity).ok_or(anyhow::anyhow!(
            "Entity {:?} does not have component {:?}",
            self.entity,
            self.component
        ))?;
        let data = &mut data[0];
        if let Some(fields) = &self.new_fields {
            for field in fields {
                data.data_mut()
                    .unwrap()
                    .set_field_by_name(field.field_name().unwrap(), field.to_owned())?;
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SelectComponent {
    pub(crate) entity: Entity,
    pub(crate) component: DynamicId,
    pub(crate) previous_entity: Option<Entity>,
    pub(crate) previous_component: Option<DynamicId>,
}

impl SelectComponent {
    pub fn new(entity: Entity, component: DynamicId) -> Self {
        Self {
            entity,
            component,
            previous_entity: None,
            previous_component: None,
        }
    }
}

impl EditorAction for SelectComponent {
    fn begin(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        self.previous_entity = state.selected_entity;
        self.previous_component = state.selected_component;
        Ok(())
    }

    fn end(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        state.selected_entity = Some(self.entity);
        state.selected_component = Some(self.component);
        Ok(())
    }

    fn undo(&mut self, state: &mut EditorState, _world: &World) -> anyhow::Result<()> {
        state.selected_entity = self.previous_entity;
        state.selected_component = self.previous_component;
        Ok(())
    }
}

#[derive(Component)]
pub struct EditorState {
    world: Arc<RwLock<World>>,

    selected_entity: Option<Entity>,
    entity_names: HashMap<Entity, String>,
    selected_component: Option<DynamicId>,

    actions_in_progress: HashMap<TypeId, Box<dyn EditorAction>>,
    action_history: Vec<Box<dyn EditorAction>>,
    undo_history: Vec<Box<dyn EditorAction>>,

    show_rename_entity: bool,
    entity_rename_buffer: String,
}

impl EditorState {
    pub fn new(world: &Arc<RwLock<World>>) -> Self {
        Self {
            world: world.clone(),

            selected_entity: None,
            selected_component: None,
            entity_names: HashMap::new(),

            actions_in_progress: HashMap::new(),
            action_history: Vec::new(),
            undo_history: Vec::new(),

            show_rename_entity: false,
            entity_rename_buffer: String::new(),
        }
    }

    pub fn perform_action<T: EditorAction>(&mut self, action: T) -> anyhow::Result<()> {
        let action = Box::new(action);
        self.begin_action(action)?;
        self.end_action::<T>()?;
        Ok(())
    }

    pub fn begin_action(&mut self, mut action: Box<dyn EditorAction>) -> anyhow::Result<()> {
        let world = self.world.clone();
        action.begin(self, &world.read())?;
        if let Some(action) = self.actions_in_progress.insert((*action).type_id(), action) {
            log::warn!("Action already in progress: {:?}", action);
        }
        Ok(())
    }

    pub fn action_in_progress<T: EditorAction>(&self) -> bool {
        self.actions_in_progress.get(&TypeId::of::<T>()).is_some()
    }

    pub fn end_action<T: EditorAction>(&mut self) -> anyhow::Result<()> {
        let world = self.world.clone();
        if let Some(mut action) = self.actions_in_progress.remove(&TypeId::of::<T>()) {
            action.end(self, &world.read())?;
            self.action_history.push(action);
        }
        Ok(())
    }

    fn update_actions(&mut self, world: &World) -> anyhow::Result<()> {
        let mut actions = std::mem::take(&mut self.actions_in_progress);
        for (type_id, mut action) in actions.drain() {
            action.update(self, world)?;
            self.actions_in_progress.insert(type_id, action);
        }
        Ok(())
    }

    pub fn undo(&mut self, world: &World) -> anyhow::Result<()> {
        if let Some(mut action) = self.action_history.pop() {
            action.undo(self, world)?;
            self.undo_history.push(action);
        }
        Ok(())
    }

    pub fn redo(&mut self, world: &World) -> anyhow::Result<()> {
        if let Some(mut action) = self.undo_history.pop() {
            action.redo(self, world)?;
            self.action_history.push(action);
        }
        Ok(())
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

    pub fn begin_rename_entity(&mut self, entity: Entity) {
        self.show_rename_entity = true;
        self.entity_rename_buffer = self.entity_name(entity).cloned().unwrap_or_default();
        self.begin_action(Box::new(RenameEntity::new(entity)))
            .unwrap();
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
        state.update_actions(&world)?;
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
    ctx.draw_if_ready(|ctx| {
        if state.show_rename_entity {
            egui::Window::new("Rename Entity")
                .default_pos(ctx.available_rect().center())
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        let response = ui.text_edit_singleline(&mut state.entity_rename_buffer);
                        if response.lost_focus() {
                            state.show_rename_entity = false;
                            state.end_action::<RenameEntity>().unwrap();
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
