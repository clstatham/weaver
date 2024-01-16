use rustc_hash::{FxHashMap, FxHashSet};
use std::ops::{Deref, DerefMut};

use super::{entity::EntityId, world::ComponentPtr};

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityComponents {
    pub(crate) components: FxHashMap<usize, ComponentPtr>,
}

impl Deref for EntityComponents {
    type Target = FxHashMap<usize, ComponentPtr>;

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
    pub entity_components: FxHashMap<EntityId, EntityComponents>,
    pub component_entities: FxHashMap<usize, FxHashSet<EntityId>>,
    pub archetypes: Vec<(FxHashSet<usize>, FxHashSet<EntityId>)>,
}

impl Components {
    pub fn add_component(&mut self, entity: EntityId, component: ComponentPtr) {
        let component_id = component.component_id;
        self.entity_components
            .entry(entity)
            .or_default()
            .insert(component_id, component.clone());
        self.component_entities
            .entry(component_id)
            .or_default()
            .insert(entity);

        self.recalculate_archetype(entity);
    }

    pub fn remove_component(&mut self, entity: EntityId, component_id: usize) {
        self.entity_components
            .get_mut(&entity)
            .and_then(|components| components.remove(&component_id));
        self.component_entities
            .get_mut(&component_id)
            .map(|entities| entities.remove(&entity));

        self.recalculate_archetype(entity);
    }

    pub fn despawn(&mut self, entity: EntityId) {
        self.entity_components.remove(&entity);
        self.component_entities
            .iter_mut()
            .for_each(|(_, entities)| {
                entities.remove(&entity);
            });

        self.recalculate_archetype(entity);
    }

    pub fn generate_archetype(&self, entity: EntityId) -> Option<FxHashSet<usize>> {
        self.entity_components
            .get(&entity)
            .map(|components| components.keys().copied().collect())
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
                    .push((archetype, FxHashSet::from_iter(vec![entity])));
            }
        }

        self.archetypes.retain(|(_, entities)| !entities.is_empty());
    }
}
