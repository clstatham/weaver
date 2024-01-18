use std::fmt::Debug;

use crate::{
    entity::EntityId,
    static_id,
    storage::{ComponentSet, EntitySet},
    Component, StaticId,
};

#[derive(Default)]
pub struct Archetype {
    pub(crate) components: ComponentSet,
    pub(crate) entities: EntitySet,
}

impl Debug for Archetype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Archetype")
            .field("entities", &self.entities)
            .finish()
    }
}

impl Archetype {
    pub fn new() -> Self {
        Self {
            components: ComponentSet::default(),
            entities: EntitySet::default(),
        }
    }

    pub fn insert_component<T: Component>(&mut self) {
        self.components.grow(static_id::<T>() as usize + 1);
        self.components.insert(static_id::<T>() as usize);
    }

    pub fn insert_raw_component(&mut self, component_id: StaticId) {
        self.components.grow(component_id as usize + 1);
        self.components.insert(component_id as usize);
    }

    pub fn insert_entity(&mut self, entity_id: EntityId) {
        self.entities.grow(entity_id as usize + 1);
        self.entities.insert(entity_id as usize);
    }

    pub fn contains_component(&self, component_id: &StaticId) -> bool {
        self.components.contains(*component_id as usize)
    }

    pub fn components(&self) -> &ComponentSet {
        &self.components
    }
}
