use std::{
    any::TypeId,
    hash::{BuildHasher, BuildHasherDefault},
    sync::atomic::{AtomicBool, AtomicU32},
};

use atomic_refcell::AtomicRefCell;
use fixedbitset::FixedBitSet;
use rustc_hash::FxHashMap;

use crate::{archetype::Archetype, query::QueryAccess, Entity, TypeIdHasher};

use super::{entity::EntityId, world::ComponentPtr};

pub type EntitySet = FixedBitSet;
pub type ComponentSet = SparseSet<TypeId, ()>;
pub type ComponentMap = SparseSet<TypeId, ComponentPtr>;
pub type EntityComponentsMap = SparseSet<EntityId, ComponentStorage>;

pub trait Index: Copy + Eq + std::hash::Hash + std::fmt::Debug {
    fn as_index(&self) -> usize;
}

impl Index for usize {
    fn as_index(&self) -> usize {
        *self
    }
}

impl Index for u32 {
    fn as_index(&self) -> usize {
        *self as usize
    }
}

impl Index for u64 {
    fn as_index(&self) -> usize {
        *self as usize
    }
}

impl Index for u128 {
    fn as_index(&self) -> usize {
        *self as usize
    }
}

impl Index for TypeId {
    fn as_index(&self) -> usize {
        BuildHasherDefault::<TypeIdHasher>::default().hash_one(*self) as usize
    }
}

#[derive(PartialEq)]
pub struct SparseSet<I: Index, T> {
    pub(crate) dense: Vec<T>,
    pub(crate) sparse: FxHashMap<I, usize>,
    pub(crate) inverse_sparse: FxHashMap<usize, I>,
}

impl<I: Index, T> SparseSet<I, T> {
    pub fn new() -> Self {
        Self {
            dense: Vec::new(),
            sparse: FxHashMap::default(),
            inverse_sparse: FxHashMap::default(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            dense: Vec::with_capacity(capacity),
            sparse: FxHashMap::default(),
            inverse_sparse: FxHashMap::default(),
        }
    }

    pub fn insert(&mut self, index: I, value: T) {
        let dense_index = self.dense.len();
        self.dense.push(value);
        self.sparse.insert(index, dense_index);
        self.inverse_sparse.insert(dense_index, index);
    }

    pub fn remove(&mut self, index: &I) -> Option<T> {
        let dense_index = *self.sparse.get(index)?;
        if dense_index >= self.dense.len() {
            return None;
        }

        if dense_index < self.dense.len() - 1 {
            let last = self.dense.len() - 1;
            self.dense.swap(dense_index, last);
            let last_index = self.inverse_sparse.remove(&last).unwrap();
            self.sparse.insert(last_index, dense_index);
            self.inverse_sparse.insert(dense_index, last_index);
        }
        self.sparse.remove(index);
        Some(self.dense.pop().unwrap())
    }

    pub fn extend(&mut self, other: &Self)
    where
        T: Clone,
    {
        for (index, value) in other.sparse.iter() {
            let value = other.dense.get(*value).unwrap();
            self.insert(*index, value.clone());
        }
    }

    pub fn get(&self, index: &I) -> Option<&T> {
        let dense_index = self.sparse.get(index)?;
        self.dense.get(*dense_index)
    }

    pub fn get_mut(&mut self, index: &I) -> Option<&mut T> {
        let dense_index = self.sparse.get(index)?;
        self.dense.get_mut(*dense_index)
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        for index in other.sparse.keys() {
            if !self.sparse.contains_key(index) {
                return false;
            }
        }
        true
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        for index in self.sparse.keys() {
            if !other.sparse.contains_key(index) {
                return false;
            }
        }
        true
    }

    pub fn contains(&self, index: &I) -> bool {
        self.sparse.contains_key(index)
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    pub fn clear(&mut self) {
        self.dense.clear();
        self.sparse.clear();
    }

    pub fn dense_iter(&self) -> impl Iterator<Item = &T> {
        self.dense.iter()
    }

    pub fn dense_iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.dense.iter_mut()
    }

    pub fn sparse_iter(&self) -> impl Iterator<Item = &I> {
        self.sparse.keys()
    }
}

impl<I: Index, T> FromIterator<(I, T)> for SparseSet<I, T> {
    fn from_iter<TIter: IntoIterator<Item = (I, T)>>(iter: TIter) -> Self {
        let mut set = Self::new();
        for (index, value) in iter {
            set.insert(index, value);
        }
        set
    }
}

impl<I: Index, T> Default for SparseSet<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ComponentStorage {
    pub entity: EntityId,
    pub component_set: ComponentSet,
    pub(crate) components: ComponentMap,
}

impl ComponentStorage {
    pub fn new(entity: EntityId) -> Self {
        Self {
            entity,
            component_set: ComponentSet::default(),
            components: ComponentMap::default(),
        }
    }

    pub fn insert(&mut self, component_id: TypeId, component: ComponentPtr) {
        self.component_set.insert(component_id, ());
        self.components.insert(component_id, component);
    }

    pub fn remove(&mut self, component_id: &TypeId) -> Option<ComponentPtr> {
        self.component_set.remove(component_id)?;
        self.components.remove(component_id)
    }

    pub fn get(&self, component_id: &TypeId) -> Option<&ComponentPtr> {
        self.components.get(component_id)
    }

    pub fn get_mut(&mut self, component_id: &TypeId) -> Option<&mut ComponentPtr> {
        self.components.get_mut(component_id)
    }

    pub fn contains_component(&self, component_id: &TypeId) -> bool {
        self.component_set.contains(component_id)
    }

    pub fn contains_components(&self, components: &ComponentSet) -> bool {
        self.component_set.is_superset(components)
    }

    pub fn keys(&self) -> impl Iterator<Item = &TypeId> {
        self.component_set.sparse.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &ComponentPtr> {
        self.components.dense_iter()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ComponentPtr> {
        self.components.dense_iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ComponentPtr> {
        self.components.dense_iter_mut()
    }

    pub fn len(&self) -> usize {
        self.component_set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.component_set.is_empty()
    }

    pub fn clear(&mut self) {
        self.component_set.clear();
        self.components.clear();
    }
}

#[derive(Default)]
pub struct Components {
    next_entity_id: AtomicU32,
    entities: EntitySet,
    pub entity_components: EntityComponentsMap,
    archetypes_dirty: AtomicBool,
    pub archetypes: AtomicRefCell<Vec<Archetype>>,
}

impl Components {
    pub fn create_entity(&mut self) -> Entity {
        let entity = self
            .next_entity_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        self.entities.grow(entity as usize + 1);
        self.entities.insert(entity as usize);

        self.entity_components
            .insert(entity, ComponentStorage::new(entity));

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
                .insert(entity, ComponentStorage::new(entity));
            self.entity_components
                .get_mut(&entity)
                .unwrap()
                .insert(component_id, component.clone());
        }

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn remove_component(&mut self, entity: EntityId, component_id: TypeId) {
        if let Some(components) = self.entity_components.get_mut(&entity) {
            components.remove(&component_id);
        }

        self.entities.set(entity as usize, false);

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
            .load(std::sync::atomic::Ordering::Acquire);

        self.next_entity_id
            .store(next_id, std::sync::atomic::Ordering::Relaxed);

        for entity in other.components.entities.ones() {
            let components = other
                .components
                .entity_components
                .remove(&(entity as EntityId))
                .unwrap();
            self.entity_components
                .insert(entity as EntityId, components);
        }

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn calculate_archetype(&self, entity: EntityId) -> Archetype {
        let mut archetype = Archetype::new();

        if let Some(components) = self.entity_components.get(&entity) {
            for component in components.iter() {
                archetype
                    .insert_raw_component(component.component_id, component.component_name.clone());
            }
        }

        archetype
    }

    pub fn fix_archetypes(&self) {
        if !self
            .archetypes_dirty
            .swap(false, std::sync::atomic::Ordering::Acquire)
        {
            return;
        }
        let mut archetypes: Vec<Archetype> = Vec::new();

        for components in self.entity_components.dense_iter() {
            let mut found = false;

            for archetype in archetypes.iter_mut() {
                if archetype.components == components.component_set {
                    archetype.insert_entity(components.entity);
                    found = true;
                    break;
                }
            }

            if !found {
                let mut archetype = self.calculate_archetype(components.entity);
                archetype.insert_entity(components.entity);
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
                entities.extend(archetype.entities.ones());
            }
        }

        entities
    }
}

#[derive(Default)]
pub struct TemporaryComponents {
    pub components: Components,
}
