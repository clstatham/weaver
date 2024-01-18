use std::{
    any::{type_name, TypeId},
    borrow::Cow,
    fmt::Debug,
};

use crate::{
    entity::EntityId,
    storage::{ComponentSet, EntitySet},
    Component, TypeIdMap,
};

#[derive(Default)]
pub struct Archetype {
    pub(crate) components: ComponentSet,
    pub(crate) component_names: TypeIdMap<Cow<'static, str>>,
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
            component_names: TypeIdMap::default(),
            entities: EntitySet::default(),
        }
    }

    pub fn insert_component<T: Component>(&mut self) {
        self.components.insert(TypeId::of::<T>(), ());
        self.component_names
            .insert(TypeId::of::<T>(), Cow::Borrowed(type_name::<T>()));
    }

    pub fn insert_raw_component(
        &mut self,
        component_id: TypeId,
        component_name: Cow<'static, str>,
    ) {
        self.components.insert(component_id, ());
        self.component_names.insert(component_id, component_name);
    }

    pub fn insert_entity(&mut self, entity_id: EntityId) {
        self.entities.grow(entity_id as usize + 1);
        self.entities.insert(entity_id as usize);
    }

    pub fn contains_component(&self, component_id: &TypeId) -> bool {
        self.components.contains(component_id)
    }

    pub fn contains_components(&self, components: &ComponentSet) -> bool {
        self.components.is_superset(components)
    }

    pub fn components(&self) -> &ComponentSet {
        &self.components
    }

    pub fn component_names(&self) -> &TypeIdMap<Cow<'static, str>> {
        &self.component_names
    }
}
