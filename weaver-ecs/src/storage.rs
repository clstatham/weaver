use rustc_hash::{FxHashMap, FxHashSet};
use std::ops::{Deref, DerefMut};

use super::{entity::EntityId, world::ComponentPtr};

pub type EntitySet = FxHashSet<EntityId>;
pub type ComponentSet = FxHashSet<u128>;
pub type ComponentMap = FxHashMap<u128, ComponentPtr>;
pub type EntityComponentsMap = FxHashMap<EntityId, ComponentMap>;
pub type QueryMap = FxHashMap<EntityId, Vec<ComponentPtr>>;

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityComponents {
    pub(crate) components: ComponentMap,
}

impl Deref for EntityComponents {
    type Target = ComponentMap;

    fn deref(&self) -> &Self::Target {
        &self.components
    }
}

impl DerefMut for EntityComponents {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.components
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Components {
    pub entity_components: EntityComponentsMap,
    pub archetypes: Vec<(ComponentSet, EntitySet)>,
}

impl Components {
    pub fn add_component(&mut self, entity: EntityId, component: ComponentPtr) {
        let component_id = component.component_id;

        if let Some(components) = self.entity_components.get_mut(&entity) {
            components.insert(component_id, component.clone());
        } else {
            self.entity_components.insert(entity, FxHashMap::default());
            self.entity_components
                .get_mut(&entity)
                .unwrap()
                .insert(component_id, component.clone());
        }

        self.recalculate_archetype(entity);
    }

    pub fn remove_component(&mut self, entity: EntityId, component_id: u128) {
        self.entity_components
            .get_mut(&entity)
            .and_then(|components| components.remove(&component_id));

        self.recalculate_archetype(entity);
    }

    pub fn despawn(&mut self, entity: EntityId) {
        self.entity_components.remove(&entity);
        self.recalculate_archetype(entity);
    }

    pub fn generate_archetype(&self, entity: EntityId) -> Option<ComponentSet> {
        self.entity_components
            .get(&entity)
            .map(|components| FxHashSet::from_iter(components.keys().copied()))
    }

    pub fn recalculate_archetype(&mut self, entity: EntityId) {
        let archetype = self.generate_archetype(entity);

        // Remove the entity from all archetypes
        self.archetypes.iter_mut().for_each(|(_, entities)| {
            entities.remove(&entity);
        });

        if let Some(archetype) = archetype {
            let mut found = false;
            for (archetype_components, entities) in self.archetypes.iter_mut() {
                if archetype_components == &archetype {
                    entities.insert(entity);
                    found = true;
                    break;
                }
            }
            if !found {
                self.archetypes
                    .push((archetype, EntitySet::from_iter(vec![entity])));
            }
        }

        self.archetypes.retain(|(_, entities)| !entities.is_empty());
    }
}
