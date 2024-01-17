use std::sync::atomic::{AtomicBool, AtomicU32};

use atomic_refcell::AtomicRefCell;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{archetype::Archetype, query::QueryAccess, Entity};

use super::{entity::EntityId, world::ComponentPtr};

pub type EntitySet = FxHashSet<EntityId>;
pub type ComponentSet = FxHashSet<u128>;
pub type ComponentMap = FxHashMap<u128, ComponentPtr>;
pub type EntityComponentsMap = FxHashMap<EntityId, ComponentStorage>;
pub type QueryMap = FxHashMap<EntityId, Vec<ComponentPtr>>;

#[derive(Default)]
pub struct ComponentStorage {
    pub set: ComponentSet,
    pub(crate) components: ComponentMap,
}

impl ComponentStorage {
    pub fn new() -> Self {
        Self {
            set: ComponentSet::default(),
            components: ComponentMap::default(),
        }
    }

    pub fn insert(&mut self, component_id: u128, component: ComponentPtr) {
        self.set.insert(component_id);
        self.components.insert(component_id, component);
    }

    pub fn remove(&mut self, component_id: &u128) -> Option<ComponentPtr> {
        if !self.set.remove(component_id) {
            return None;
        }
        self.components.remove(component_id)
    }

    pub fn get(&self, component_id: &u128) -> Option<&ComponentPtr> {
        self.components.get(component_id)
    }

    pub fn get_mut(&mut self, component_id: &u128) -> Option<&mut ComponentPtr> {
        self.components.get_mut(component_id)
    }

    pub fn contains_component(&self, component_id: &u128) -> bool {
        self.set.contains(component_id)
    }

    pub fn contains_components(&self, components: &ComponentSet) -> bool {
        self.set.is_superset(components)
    }

    pub fn union_with(&mut self, other: &Self) {
        self.set.extend(&other.set);
        self.components.extend(other.components.clone());
    }

    pub fn intersection_with(&mut self, other: &Self) {
        self.set = self.set.intersection(&other.set).copied().collect();
        self.components
            .retain(|k, _| other.components.contains_key(k));
    }

    pub fn difference_with(&mut self, other: &Self) {
        self.set = self.set.difference(&other.set).copied().collect();
        self.components
            .retain(|k, _| !other.components.contains_key(k));
    }

    pub fn keys(&self) -> impl Iterator<Item = &u128> {
        self.set.iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &ComponentPtr> {
        self.components.values()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u128, &ComponentPtr)> {
        self.components.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&u128, &mut ComponentPtr)> {
        self.components.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    pub fn clear(&mut self) {
        self.set.clear();
        self.components.clear();
    }
}

#[derive(Default)]
pub struct Components {
    next_entity_id: AtomicU32,
    pub entity_components: EntityComponentsMap,
    archetypes_dirty: AtomicBool,
    pub archetypes: AtomicRefCell<Vec<Archetype>>,
}

impl Components {
    pub fn create_entity(&mut self) -> Entity {
        let entity = self
            .next_entity_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        self.entity_components
            .insert(entity, ComponentStorage::default());

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);

        Entity::new(entity, 0)
    }

    pub fn add_component(&mut self, entity: EntityId, component: ComponentPtr) {
        let component_id = component.component_id;

        if let Some(components) = self.entity_components.get_mut(&entity) {
            components.insert(component_id, component.clone());
        } else {
            self.entity_components
                .insert(entity, ComponentStorage::default());
            self.entity_components
                .get_mut(&entity)
                .unwrap()
                .insert(component_id, component.clone());
        }

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn remove_component(&mut self, entity: EntityId, component_id: u128) {
        self.entity_components
            .get_mut(&entity)
            .and_then(|components| components.remove(&component_id));

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn despawn(&mut self, entity: EntityId) {
        self.entity_components.remove(&entity);

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn split(&self) -> TemporaryComponents {
        let id = self
            .next_entity_id
            .load(std::sync::atomic::Ordering::Relaxed);

        let components = TemporaryComponents::default();

        components
            .components
            .next_entity_id
            .store(id, std::sync::atomic::Ordering::Relaxed);

        components
    }

    pub(crate) fn merge(&mut self, mut other: TemporaryComponents) {
        let next_id = other
            .components
            .next_entity_id
            .load(std::sync::atomic::Ordering::Relaxed);

        self.next_entity_id
            .store(next_id, std::sync::atomic::Ordering::Relaxed);

        for (entity, components) in other.components.entity_components.drain() {
            self.entity_components.insert(entity, components);
        }

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn calculate_archetype(&self, entity: EntityId) -> Archetype {
        let mut archetype = Archetype::new();

        if let Some(components) = self.entity_components.get(&entity) {
            for (component_id, component) in components.iter() {
                archetype.insert_raw_component(*component_id, component.component_name.clone());
            }
        }

        archetype
    }

    pub fn fix_archetypes(&self) {
        if !self
            .archetypes_dirty
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        let mut archetypes: Vec<Archetype> = Vec::new();

        for (entity, components) in self.entity_components.iter() {
            let mut found = false;

            for archetype in archetypes.iter_mut() {
                if archetype.components == components.set {
                    archetype.insert_entity(*entity);
                    found = true;
                    break;
                }
            }

            if !found {
                let mut archetype = self.calculate_archetype(*entity);
                archetype.insert_entity(*entity);
                archetypes.push(archetype);
            }
        }

        *self.archetypes.borrow_mut() = archetypes;
    }

    pub fn entities_matching_access(&self, access: &QueryAccess) -> EntitySet {
        self.fix_archetypes();

        let mut entities = EntitySet::default();

        for archetype in self.archetypes.borrow().iter() {
            if access.matches_archetype(archetype) {
                entities.extend(archetype.entities.iter());
            }
        }

        entities
    }
}

#[derive(Default)]
pub struct TemporaryComponents {
    pub components: Components,
}
