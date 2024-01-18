use std::{
    fmt::Debug,
    sync::atomic::{AtomicBool, AtomicU32},
};

use atomic_refcell::AtomicRefCell;
use fixedbitset::FixedBitSet;

use crate::{archetype::Archetype, query::QueryAccess, Entity, StaticId};

use super::{component::ComponentPtr, entity::EntityId};

pub type EntitySet = FixedBitSet;
pub type ComponentSet = FixedBitSet;
pub type ComponentMap = SparseSet<u64, ComponentPtr>;
pub type EntityComponentsMap = SparseSet<EntityId, ComponentStorage>;

pub trait Index: Copy + Eq + std::hash::Hash + std::fmt::Debug {
    fn as_index(&self) -> usize;
    fn from_index(index: usize) -> Self;
}

impl Index for usize {
    fn as_index(&self) -> usize {
        *self
    }

    fn from_index(index: usize) -> Self {
        index
    }
}

impl Index for u32 {
    fn as_index(&self) -> usize {
        *self as usize
    }

    fn from_index(index: usize) -> Self {
        index as u32
    }
}

impl Index for u64 {
    fn as_index(&self) -> usize {
        *self as usize
    }

    fn from_index(index: usize) -> Self {
        index as u64
    }
}

pub trait HasIndex {
    type Index: Index;

    fn index(&self) -> Self::Index;
}

#[derive(PartialEq)]
pub struct SparseSet<I: Index, T: HasIndex> {
    pub(crate) dense: Vec<T>,
    pub(crate) sparse: Vec<Option<usize>>,
    pub(crate) indices: FixedBitSet,
    _marker: std::marker::PhantomData<I>,
}

impl<I: Index, T: HasIndex> SparseSet<I, T> {
    pub fn new() -> Self {
        Self {
            dense: Vec::new(),
            sparse: Vec::new(),
            indices: FixedBitSet::new(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            dense: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
            indices: FixedBitSet::with_capacity(capacity),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn insert(&mut self, index: I, value: T) {
        self.dense.push(value);
        let dense_index = self.dense.len() - 1;
        if index.as_index() >= self.sparse.len() {
            self.sparse.resize(index.as_index() + 1, None);
        }
        self.sparse[index.as_index()] = Some(dense_index);
        self.indices.grow(index.as_index() + 1);
        self.indices.set(index.as_index(), true);
    }

    pub fn remove(&mut self, index: I) -> Option<T> {
        let dense_index = *self.sparse.get(index.as_index())?;
        let dense_index = dense_index?;
        if dense_index >= self.dense.len() {
            return None;
        }

        let last_dense_index = self.dense.len() - 1;

        self.dense.swap(dense_index, last_dense_index);
        // the dense index is now invalid, so we need to update the sparse set
        self.sparse[self.dense[dense_index].index().as_index()] = Some(dense_index);
        self.indices
            .set(self.dense[dense_index].index().as_index(), true);
        self.sparse[self.dense[last_dense_index].index().as_index()] = None;
        self.indices
            .set(self.dense[last_dense_index].index().as_index(), false);

        self.dense.pop()
    }

    pub fn get(&self, index: &I) -> Option<&T> {
        let dense_index = self.sparse.get(index.as_index()).copied()??;
        // if the dense index is invalid, it's always a bug
        Some(self.dense.get(dense_index).unwrap())
    }

    pub fn get_mut(&mut self, index: &I) -> Option<&mut T> {
        let dense_index = self.sparse.get(index.as_index()).copied()??;
        // if the dense index is invalid, it's always a bug
        Some(self.dense.get_mut(dense_index).unwrap())
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        self.indices.is_superset(&other.indices)
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.indices.is_subset(&other.indices)
    }

    pub fn contains(&self, index: &I) -> bool {
        self.indices.contains(index.as_index())
    }

    pub fn clear(&mut self) {
        self.dense.clear();
        self.sparse.clear();
        self.indices.clear();
    }

    pub fn dense_iter(&self) -> impl Iterator<Item = &T> {
        self.dense.iter()
    }

    pub fn dense_iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.dense.iter_mut()
    }

    pub fn sparse_iter(&self) -> impl Iterator<Item = I> + '_ {
        self.indices.ones().map(I::from_index)
    }

    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> + '_ {
        self.sparse_iter()
            .map(move |index| (index, self.get(&index).unwrap()))
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }
}

impl<I: Index, T: HasIndex> FromIterator<(I, T)> for SparseSet<I, T> {
    fn from_iter<TIter: IntoIterator<Item = (I, T)>>(iter: TIter) -> Self {
        let mut set = Self::new();
        for (index, value) in iter {
            set.insert(index, value);
        }
        set
    }
}

impl<I: Index, T: HasIndex> Default for SparseSet<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Index, T: HasIndex> Debug for SparseSet<I, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SparseSet")
            .field(
                "sparse",
                &self
                    .sparse
                    .iter()
                    .enumerate()
                    .filter_map(|x| x.1.map(|_| (x.0)))
                    .collect::<Vec<_>>(),
            )
            .field("indices", &self.indices.ones().collect::<Vec<_>>())
            .finish()
    }
}

impl HasIndex for ComponentPtr {
    type Index = u64;

    fn index(&self) -> Self::Index {
        self.type_info.id
    }
}

pub struct ComponentStorage {
    pub entity: EntityId,
    pub(crate) components: ComponentMap,
}

impl HasIndex for ComponentStorage {
    type Index = EntityId;

    fn index(&self) -> Self::Index {
        self.entity
    }
}

impl Debug for ComponentStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentStorage")
            .field("entity", &self.entity)
            .field("components", &self.components)
            .finish()
    }
}

impl ComponentStorage {
    pub fn new(entity: EntityId) -> Self {
        Self {
            entity,
            components: ComponentMap::default(),
        }
    }

    pub fn insert(&mut self, component_id: StaticId, component: ComponentPtr) {
        self.components.insert(component_id, component);
    }

    pub fn remove(&mut self, component_id: StaticId) -> Option<ComponentPtr> {
        self.components.remove(component_id)
    }

    pub fn get(&self, component_id: &StaticId) -> Option<&ComponentPtr> {
        self.components.get(component_id)
    }

    pub fn get_mut(&mut self, component_id: &StaticId) -> Option<&mut ComponentPtr> {
        self.components.get_mut(component_id)
    }

    pub fn contains_component(&self, component_id: &StaticId) -> bool {
        self.components.contains(component_id)
    }

    pub fn keys(&self) -> impl Iterator<Item = StaticId> + '_ {
        self.components.sparse_iter()
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

    pub fn clear(&mut self) {
        self.components.clear();
    }
}

#[derive(Default)]
pub struct Components {
    next_entity_id: AtomicU32,
    entities: EntitySet,
    pub entity_components: EntityComponentsMap,
    archetypes_dirty: AtomicBool,
    archetypes: AtomicRefCell<Vec<Archetype>>,
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
        let component_id = component.id();

        if let Some(components) = self.entity_components.get_mut(&entity) {
            components.insert(component_id, component);
        } else {
            self.entity_components
                .insert(entity, ComponentStorage::new(entity));

            self.entity_components
                .get_mut(&entity)
                .unwrap()
                .insert(component_id, component);
        }

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn remove_component(&mut self, entity: EntityId, component_id: StaticId) {
        if let Some(components) = self.entity_components.get_mut(&entity) {
            components.remove(component_id);
        }

        self.entities.set(entity as usize, false);

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn despawn(&mut self, entity: EntityId) {
        self.entity_components.remove(entity);

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

        for entity in other.components.entities.ones() {
            let components = other
                .components
                .entity_components
                .remove(entity as EntityId)
                .unwrap();
            self.entity_components
                .insert(entity as EntityId, components);
        }

        self.archetypes_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn calculate_archetype(&self, entity: EntityId) -> Option<Archetype> {
        if let Some(components) = self.entity_components.get(&entity) {
            let mut archetype = Archetype::new();
            archetype
                .components
                .union_with(&components.components.indices);
            archetype.insert_entity(entity);
            Some(archetype)
        } else {
            None
        }
    }

    #[cfg_attr(feature = "bench", inline(never))]
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
                if archetype.components.count_ones(..) == components.components.len()
                    && archetype
                        .components
                        .ones()
                        .zip(components.components.sparse_iter())
                        .all(|(a, b)| a == b as usize)
                {
                    archetype.insert_entity(components.entity);
                    found = true;
                    break;
                }
            }

            if !found {
                let archetype = self.calculate_archetype(components.entity).unwrap();
                archetypes.push(archetype);
            }
        }

        *self.archetypes.borrow_mut() = archetypes;
    }

    #[cfg_attr(feature = "bench", inline(never))]
    pub fn entities_matching_access(&self, access: &QueryAccess) -> EntitySet {
        self.fix_archetypes();

        let mut entities = EntitySet::default();

        for archetype in self.archetypes.borrow().iter() {
            if access.matches_archetype(archetype) {
                entities.union_with(&archetype.entities);
            }
        }

        entities
    }
}

#[derive(Default)]
pub struct TemporaryComponents {
    pub components: Components,
}
