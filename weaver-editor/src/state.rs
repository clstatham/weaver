use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use weaver::prelude::*;

use crate::ui::scene_tree::NameTag;

pub trait EditorAction: Send + Sync + Any + 'static {
    fn begin(&mut self, world: &LockedWorldHandle, state: &mut EditorState) -> anyhow::Result<()>;
    #[allow(unused_variables)]
    fn update(&mut self, world: &LockedWorldHandle, state: &mut EditorState) -> anyhow::Result<()> {
        Ok(())
    }
    fn end(&mut self, world: &LockedWorldHandle, state: &mut EditorState) -> anyhow::Result<()>;
    fn undo(&mut self, world: &LockedWorldHandle, state: &mut EditorState) -> anyhow::Result<()>;
    fn redo(&mut self, world: &LockedWorldHandle, state: &mut EditorState) -> anyhow::Result<()> {
        self.begin(world, state)?;
        self.end(world, state)?;
        Ok(())
    }
}

pub struct RenameEntity {
    pub entity: Entity,
    pub old_name: String,
    pub new_name: String,
}

impl EditorAction for RenameEntity {
    fn begin(&mut self, world: &LockedWorldHandle, _state: &mut EditorState) -> anyhow::Result<()> {
        if self.entity.has::<NameTag>(world) {
            self.old_name = self
                .entity
                .with_component_ref::<NameTag, _>(world, |tag| tag.0.clone())
                .unwrap();
        } else {
            self.old_name = self
                .entity
                .type_name()
                .unwrap_or_else(|| format!("{}", self.entity.id()));
        }
        Ok(())
    }

    fn end(&mut self, world: &LockedWorldHandle, _state: &mut EditorState) -> anyhow::Result<()> {
        if self.entity.has::<NameTag>(world) {
            self.entity
                .with_component_mut::<NameTag, _>(world, |mut tag| tag.0 = self.new_name.clone());
        } else {
            self.entity.add(world, NameTag(self.new_name.clone()))?;
        }
        Ok(())
    }

    fn undo(&mut self, world: &LockedWorldHandle, _state: &mut EditorState) -> anyhow::Result<()> {
        if self.entity.has::<NameTag>(world) {
            self.entity
                .with_component_mut::<NameTag, _>(world, |mut tag| tag.0 = self.old_name.clone());
        } else {
            self.entity.add(world, NameTag(self.old_name.clone()))?;
        }
        Ok(())
    }
}

#[derive(Component)]
pub struct EditorState {
    pub(crate) selected_entity: Option<Entity>,
    pub(crate) selected_component: Option<Entity>,

    actions_in_progress: HashMap<TypeId, Box<dyn EditorAction>>,
    action_history: Vec<Box<dyn EditorAction>>,
    undo_history: Vec<Box<dyn EditorAction>>,

    pub(crate) entity_being_renamed: Option<Entity>,
    pub(crate) entity_rename_buffer: String,

    pub(crate) viewport_id: Option<egui::epaint::TextureId>,
}

impl Clone for EditorState {
    fn clone(&self) -> Self {
        unimplemented!("Clone not implemented for EditorState")
    }
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            selected_entity: None,
            selected_component: None,

            actions_in_progress: HashMap::new(),
            action_history: Vec::new(),
            undo_history: Vec::new(),

            entity_being_renamed: None,
            entity_rename_buffer: String::new(),

            viewport_id: None,
        }
    }

    pub fn perform_action<T: EditorAction>(
        &mut self,
        world: &LockedWorldHandle,
        action: T,
    ) -> anyhow::Result<()> {
        let action = Box::new(action);
        self.begin_action(world, action)?;
        self.end_action::<T>(world)?;
        Ok(())
    }

    pub fn begin_action(
        &mut self,
        world: &LockedWorldHandle,
        mut action: Box<dyn EditorAction>,
    ) -> anyhow::Result<()> {
        action.begin(world, self)?;
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

    pub fn end_action<T: EditorAction>(&mut self, world: &LockedWorldHandle) -> anyhow::Result<()> {
        if let Some(mut action) = self.actions_in_progress.remove(&TypeId::of::<T>()) {
            action.end(world, self)?;
            self.action_history.push(action);
        }
        Ok(())
    }

    pub fn undo(&mut self, world: &LockedWorldHandle) -> anyhow::Result<()> {
        if let Some(mut action) = self.action_history.pop() {
            action.undo(world, self)?;
            self.undo_history.push(action);
        }
        Ok(())
    }

    pub fn redo(&mut self, world: &LockedWorldHandle) -> anyhow::Result<()> {
        if let Some(mut action) = self.undo_history.pop() {
            action.redo(world, self)?;
            self.action_history.push(action);
        }
        Ok(())
    }

    pub fn rename_entity_window(
        &mut self,
        world: &LockedWorldHandle,
        ctx: &egui::Context,
    ) -> anyhow::Result<()> {
        if self.entity_being_renamed.is_some() {
            egui::Window::new("Rename Entity")
                .default_pos(ctx.screen_rect().center())
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut self.entity_rename_buffer);
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.entity_being_renamed = None;
                        }
                        if ui.button("Rename").clicked() {
                            if let Some(entity) = self.entity_being_renamed.take() {
                                self.perform_action(
                                    world,
                                    RenameEntity {
                                        entity,
                                        old_name: String::new(),
                                        new_name: self.entity_rename_buffer.clone(),
                                    },
                                )
                                .unwrap();

                                self.entity_rename_buffer.clear();
                            }
                        }
                    });
                });
        }
        Ok(())
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}
