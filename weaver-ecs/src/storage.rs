use std::{fmt::Debug, sync::atomic::AtomicU32};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use fixedbitset::FixedBitSet;

use crate::{component::Data, query::QueryAccess, Bundle, Component, Entity, StaticId, TypeInfo};

use super::entity::EntityId;

pub type EntitySet = FixedBitSet;
pub type ComponentSet = FixedBitSet;

pub trait Index: Copy + Debug + Eq + Ord + std::hash::Hash {
    fn into_usize(self) -> usize;
    fn from_usize(index: usize) -> Self;
}

impl Index for usize {
    fn into_usize(self) -> usize {
        self
    }
    fn from_usize(index: usize) -> Self {
        index
    }
}
impl Index for StaticId {
    fn into_usize(self) -> usize {
        self as usize
    }
    fn from_usize(index: usize) -> Self {
        index as StaticId
    }
}
impl Index for EntityId {
    fn into_usize(self) -> usize {
        self as usize
    }
    fn from_usize(index: usize) -> Self {
        index as EntityId
    }
}

pub trait HasIndex {
    type Index;

    fn index(&self) -> Self::Index;
}

/// A sparse set of values indexed by a dense set of indices.
/// This is used to store components in an archetype.
pub struct SparseSet<I: Index, V> {
    indices: Vec<I>,
    dense: Vec<V>,
    sparse: Vec<Option<usize>>,
}

impl<I: Index, V> Default for SparseSet<I, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Index, V> SparseSet<I, V> {
    pub fn new() -> Self {
        Self {
            indices: Vec::new(),
            dense: Vec::new(),
            sparse: Vec::new(),
        }
    }

    pub fn dense_index_of(&self, index: &I) -> Option<usize> {
        let index = index.into_usize();

        if index >= self.sparse.len() {
            return None;
        }

        self.sparse[index]
    }

    pub fn insert(&mut self, index: I, value: V) {
        match self.dense_index_of(&index) {
            Some(dense_index) => {
                self.dense[dense_index] = value;
            }
            None => {
                let dense_index = self.dense.len();

                self.indices.push(index);
                self.dense.push(value);
                if index.into_usize() >= self.sparse.len() {
                    self.sparse.resize(index.into_usize() + 1, None);
                }
                self.sparse[index.into_usize()] = Some(dense_index);
            }
        }
    }

    pub fn remove(&mut self, index: I) -> Option<V> {
        if index.into_usize() >= self.sparse.len() {
            return None;
        }
        let dense_index = self.sparse[index.into_usize()].take()?;

        let value = self.dense.swap_remove(dense_index);
        let _index = self.indices.swap_remove(dense_index);

        if dense_index != self.dense.len() {
            let swapped_index = self.indices[dense_index];
            self.sparse[swapped_index.into_usize()] = Some(dense_index);
        }

        Some(value)
    }

    pub fn get(&self, index: &I) -> Option<&V> {
        let index = index.into_usize();

        self.sparse
            .get(index)
            .copied()
            .flatten()
            .map(|index| self.dense.get(index).unwrap())
    }

    pub fn get_mut(&mut self, index: &I) -> Option<&mut V> {
        let index = index.into_usize();

        self.sparse
            .get(index)
            .copied()
            .flatten()
            .map(|index| self.dense.get_mut(index).unwrap())
    }

    pub fn dense_iter(&self) -> impl Iterator<Item = &V> {
        self.dense.iter()
    }

    pub fn dense_iter_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.dense.iter_mut()
    }

    pub fn sparse_iter(&self) -> impl Iterator<Item = I> + '_ {
        self.indices.iter().copied()
    }

    pub fn contains(&self, index: &I) -> bool {
        let index = index.into_usize();

        if index >= self.sparse.len() {
            return false;
        }

        self.sparse[index].is_some()
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    pub fn clear(&mut self) {
        self.indices.clear();
        self.dense.clear();
        self.sparse.clear();
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        self.indices.iter().all(|index| other.contains(index))
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        other.is_superset(self)
    }
}

/// A single "column" of an archetypal table.
pub struct Column {
    pub(crate) info: TypeInfo,
    pub(crate) storage: SparseSet<EntityId, Data>,
}

impl HasIndex for Column {
    type Index = StaticId;

    fn index(&self) -> Self::Index {
        self.info.id
    }
}

impl Debug for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentStorage")
            .field("component_id", &self.info.id())
            .field("entities", &self.storage.sparse_iter().collect::<Vec<_>>())
            .finish()
    }
}

impl Column {
    pub fn new<T: Component>() -> Self {
        Self {
            info: TypeInfo::of::<T>(),
            storage: SparseSet::new(),
        }
    }

    pub fn new_with_info(info: TypeInfo) -> Self {
        Self {
            info,
            storage: SparseSet::new(),
        }
    }

    pub fn insert(&mut self, entity: EntityId, component: Data) {
        assert_eq!(self.info.id, component.id());
        self.storage.insert(entity, component);
    }

    pub fn remove(&mut self, entity: EntityId) -> Option<Data> {
        self.storage.remove(entity)
    }

    pub fn get(&self, entity: EntityId) -> Option<&Data> {
        self.storage.get(&entity)
    }

    pub fn get_mut(&mut self, entity: EntityId) -> Option<&mut Data> {
        self.storage.get_mut(&entity)
    }
}

/// A unique combination of components. Also maintains a list of entities that have this combination.
#[derive(Default)]
pub struct Archetype {
    pub(crate) entities: EntitySet,
    pub(crate) component_types: Box<[TypeInfo]>,
    pub(crate) columns: SparseSet<StaticId, AtomicRefCell<Column>>,
}

impl Archetype {
    pub fn with_component_types(component_types: Vec<TypeInfo>) -> Self {
        let mut columns = SparseSet::new();

        for info in component_types.iter() {
            columns.insert(info.id(), AtomicRefCell::new(Column::new_with_info(*info)));
        }

        Self {
            entities: EntitySet::default(),
            component_types: component_types.into_boxed_slice(),
            columns,
        }
    }

    pub(crate) fn insert_entity(&mut self, entity: EntityId, component: Data) {
        self.entities.grow(entity as usize + 1);
        self.entities.insert(entity as usize);
        self.columns
            .get_mut(&component.id())
            .unwrap()
            .borrow_mut()
            .insert(entity, component);
    }

    pub(crate) fn remove_entity(&mut self, entity: EntityId) {
        self.entities.set(entity as usize, false);

        for column in self.columns.dense_iter_mut() {
            column.borrow_mut().remove(entity);
        }
    }

    pub fn exclusively_contains_components(&self, component_ids: &ComponentSet) -> bool {
        component_ids
            .ones()
            .all(|index| self.columns.contains(&(index as StaticId)))
            && self.columns.indices.len() == component_ids.count_ones(..)
    }

    pub fn contains_entity(&self, entity: EntityId) -> bool {
        self.entities.contains(entity as usize)
    }

    pub fn get_column(&self, component_id: StaticId) -> Option<AtomicRef<'_, Column>> {
        self.columns.get(&component_id).map(|x| x.borrow())
    }

    pub fn get_column_mut(&self, component_id: StaticId) -> Option<AtomicRefMut<'_, Column>> {
        self.columns.get(&component_id).map(|x| x.borrow_mut())
    }

    pub fn component_drain(&mut self, entity: EntityId) -> impl Iterator<Item = Data> + '_ {
        self.entities.set(entity as usize, false);
        self.columns
            .dense_iter_mut()
            .filter_map(move |column| column.borrow_mut().remove(entity))
    }

    pub fn component_ids(&self) -> ComponentSet {
        self.columns
            .indices
            .iter()
            .map(|x| x.into_usize())
            .collect::<ComponentSet>()
    }
}

#[derive(Default)]
pub struct Components {
    next_entity_id: AtomicU32,
    entities: EntitySet,
    pub archetypes: Vec<Archetype>,
}

impl Components {
    pub fn create_entity(&mut self) -> Entity {
        let entity = self
            .next_entity_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        self.entities.grow(entity as usize + 1);
        self.entities.insert(entity as usize);

        Entity::new(entity, 0)
    }

    pub fn build<B: Bundle>(&mut self, bundle: B) -> Entity {
        // add the components in bulk
        let components = bundle.components();
        let component_infos = B::component_infos();

        self.build_with_components(components, component_infos)
    }

    pub fn build_with_components(
        &mut self,
        components: Vec<Data>,
        component_infos: Vec<TypeInfo>,
    ) -> Entity {
        let entity = self.create_entity();
        self.build_on_with_components(entity.id(), components, component_infos);
        entity
    }

    pub fn build_on_with_components(
        &mut self,
        entity: EntityId,
        mut components: Vec<Data>,
        component_infos: Vec<TypeInfo>,
    ) {
        let component_ids = component_infos
            .iter()
            .map(|info| info.id as usize)
            .collect::<ComponentSet>();

        // find the archetype that matches the components, if any
        let mut found = false;
        for archetype in self.archetypes.iter_mut() {
            if archetype.exclusively_contains_components(&component_ids) {
                for component in components.drain(..) {
                    archetype.insert_entity(entity, component);
                }
                found = true;
                break;
            }
        }

        if !found {
            // if we didn't find an archetype, create a new one
            let mut archetype = Archetype::with_component_types(component_infos);

            if let Some(old_archetype) = self.find_archetype_mut(entity) {
                for component in old_archetype.component_drain(entity) {
                    archetype.insert_entity(entity, component);
                }
                old_archetype.remove_entity(entity);
            }

            for component in components {
                archetype.insert_entity(entity, component);
            }

            self.archetypes.push(archetype);
        }
    }

    pub fn despawn(&mut self, entity: EntityId) {
        let archetype = self.find_archetype_mut(entity).unwrap();
        archetype.remove_entity(entity);
    }

    pub fn find_archetype(&self, entity: EntityId) -> Option<&Archetype> {
        self.archetypes
            .iter()
            .find(|archetype| archetype.contains_entity(entity))
    }

    pub fn find_archetype_mut(&mut self, entity: EntityId) -> Option<&mut Archetype> {
        self.archetypes
            .iter_mut()
            .find(|archetype| archetype.contains_entity(entity))
    }

    pub fn split(&self) -> TemporaryComponents {
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

    pub fn merge(&mut self, mut other: TemporaryComponents) {
        let next_id = other
            .components
            .next_entity_id
            .load(std::sync::atomic::Ordering::Relaxed);

        self.next_entity_id
            .store(next_id, std::sync::atomic::Ordering::Relaxed);

        for entity in other.components.entities.ones().collect::<Vec<_>>() {
            if !self.entities.contains(entity) {
                let archetype = other
                    .components
                    .find_archetype_mut(entity as EntityId)
                    .unwrap();

                let components = archetype
                    .component_drain(entity as EntityId)
                    .collect::<Vec<_>>();

                let component_infos = archetype.component_types.to_vec();

                self.build_on_with_components(entity as EntityId, components, component_infos);
            }
        }
    }

    pub fn entities_matching_access(&self, access: &QueryAccess) -> EntitySet {
        let mut entities = EntitySet::default();

        for archetype in self.archetypes.iter() {
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
