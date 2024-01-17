use std::{borrow::Cow, fmt::Debug};

use rustc_hash::FxHashMap;

use crate::{
    entity::EntityId,
    storage::{ComponentSet, EntitySet},
    Component,
};

#[derive(Default)]
pub struct Archetype {
    pub(crate) components: ComponentSet,
    pub(crate) component_names: FxHashMap<u128, Cow<'static, str>>,
    pub(crate) entities: EntitySet,
}

impl Debug for Archetype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Archetype")
            .field(
                "component_names",
                &self.component_names.values().collect::<Vec<_>>(),
            )
            .field("entities", &self.entities)
            .finish()
    }
}

impl Archetype {
    pub fn new() -> Self {
        Self {
            components: ComponentSet::default(),
            component_names: FxHashMap::default(),
            entities: EntitySet::default(),
        }
    }

    pub fn insert_component<T: Component>(&mut self) {
        self.components.insert(T::static_id());
        self.component_names
            .insert(T::static_id(), Cow::Borrowed(T::static_name()));
    }

    pub fn insert_raw_component(&mut self, component_id: u128, component_name: Cow<'static, str>) {
        self.components.insert(component_id);
        self.component_names.insert(component_id, component_name);
    }

    pub fn insert_entity(&mut self, entity: EntityId) {
        self.entities.insert(entity);
    }

    pub fn remove_entity(&mut self, entity: &EntityId) -> bool {
        self.entities.remove(entity)
    }

    pub fn contains_entity(&self, entity: &EntityId) -> bool {
        self.entities.contains(entity)
    }

    pub fn contains_component(&self, component_id: &u128) -> bool {
        self.components.contains(component_id)
    }

    pub fn contains_components(&self, components: &ComponentSet) -> bool {
        self.components.is_superset(components)
    }

    pub fn components(&self) -> &ComponentSet {
        &self.components
    }

    pub fn component_names(&self) -> &FxHashMap<u128, Cow<'static, str>> {
        &self.component_names
    }

    pub fn entities(&self) -> &EntitySet {
        &self.entities
    }
}
