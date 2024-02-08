use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use weaver::prelude::*;

pub trait EditorAction: Send + Sync + Any + 'static {
    fn begin(&mut self, state: &mut EditorState, world: &mut World) -> anyhow::Result<()>;
    #[allow(unused_variables)]
    fn update(&mut self, state: &mut EditorState, world: &mut World) -> anyhow::Result<()> {
        Ok(())
    }
    fn end(&mut self, state: &mut EditorState, world: &mut World) -> anyhow::Result<()>;
    fn undo(&mut self, state: &mut EditorState, world: &mut World) -> anyhow::Result<()>;
    fn redo(&mut self, state: &mut EditorState, world: &mut World) -> anyhow::Result<()> {
        self.begin(state, world)?;
        self.end(state, world)?;
        Ok(())
    }
}

#[derive(Atom)]
pub struct EditorState {
    pub(crate) world: LockedWorldHandle,

    pub(crate) selected_entity: Option<Entity>,
    entity_names: HashMap<Entity, String>,
    pub(crate) selected_component: Option<Entity>,

    actions_in_progress: HashMap<TypeId, Box<dyn EditorAction>>,
    action_history: Vec<Box<dyn EditorAction>>,
    undo_history: Vec<Box<dyn EditorAction>>,

    pub(crate) show_rename_entity: bool,
    pub(crate) entity_rename_buffer: String,
}

impl Clone for EditorState {
    fn clone(&self) -> Self {
        unimplemented!("Clone not implemented for EditorState")
    }
}

impl EditorState {
    pub fn new(world: &LockedWorldHandle) -> Self {
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
        action.begin(self, &mut world.write())?;
        if self
            .actions_in_progress
            .insert((*action).type_id(), action)
            .is_some()
        {
            log::warn!("Action already in progress");
        }
        Ok(())
    }

    pub fn action_in_progress<T: EditorAction>(&self) -> bool {
        self.actions_in_progress.get(&TypeId::of::<T>()).is_some()
    }

    pub fn end_action<T: EditorAction>(&mut self) -> anyhow::Result<()> {
        let world = self.world.clone();
        if let Some(mut action) = self.actions_in_progress.remove(&TypeId::of::<T>()) {
            action.end(self, &mut world.write())?;
            self.action_history.push(action);
        }
        Ok(())
    }
}
