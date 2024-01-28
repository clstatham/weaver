use std::{fmt::Debug, sync::Arc};

use crate::{
    bundle::Bundle,
    component::Component,
    component::Data,
    entity::Entity,
    query::QueryAccess,
    registry::{DynamicId, Registry, SortedMap},
};

pub type EntitySet = SparseSet<Entity, ()>;
pub type ArchetypeSet = SparseSet<usize, ()>;
pub type ComponentSet = SparseSet<DynamicId, ()>;
pub type ComponentMap<T> = SparseSet<DynamicId, T>;

pub trait Index: Debug + Eq + Ord + std::hash::Hash {
    fn as_usize(&self) -> usize;
}

impl Index for usize {
    fn as_usize(&self) -> usize {
        *self
    }
}
impl Index for DynamicId {
    fn as_usize(&self) -> usize {
        *self as usize
    }
}
impl Index for Entity {
    fn as_usize(&self) -> usize {
        self.id() as usize
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

impl<I: Index, V: Debug> Debug for SparseSet<I, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SparseSet")
            .field("indices", &self.indices)
            .field("dense", &self.dense)
            .finish()
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

    #[inline]
    pub fn dense_index_of(&self, index: &I) -> Option<usize> {
        let index = index.as_usize();

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

                if index.as_usize() >= self.sparse.len() {
                    self.sparse.resize(index.as_usize() + 1, None);
                }
                self.sparse[index.as_usize()] = Some(dense_index);
                self.indices.push(index);
                self.dense.push(value);
            }
        }
    }

    pub fn remove(&mut self, index: &I) -> Option<V> {
        if index.as_usize() >= self.sparse.len() {
            return None;
        }
        let dense_index = self.sparse[index.as_usize()].take()?;

        let value = self.dense.swap_remove(dense_index);
        let _index = self.indices.swap_remove(dense_index);

        if dense_index != self.dense.len() {
            let swapped_index = &self.indices[dense_index];
            self.sparse[swapped_index.as_usize()] = Some(dense_index);
        }

        Some(value)
    }

    pub fn get(&self, index: &I) -> Option<&V> {
        let index = index.as_usize();

        self.sparse
            .get(index)
            .copied()
            .flatten()
            .map(|index| self.dense.get(index).unwrap())
    }

    pub fn get_mut(&mut self, index: &I) -> Option<&mut V> {
        let index = index.as_usize();

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

    pub fn sparse_iter(&self) -> impl Iterator<Item = &I> + '_ {
        self.indices.iter()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&I, &V)> + '_ {
        self.indices.iter().zip(self.dense.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&I, &mut V)> + '_ {
        self.indices.iter().zip(self.dense.iter_mut())
    }

    pub fn contains(&self, index: &I) -> bool {
        let index = index.as_usize();

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

    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.indices.iter().all(|index| !other.contains(index))
    }

    pub fn intersection<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = (&I, &V)> + '_ {
        self.indices
            .iter()
            .filter_map(move |index| other.get(index).map(|value| (index, value)))
    }
}

impl<I: Index, V> IntoIterator for SparseSet<I, V> {
    type Item = (I, V);
    type IntoIter = std::iter::Zip<std::vec::IntoIter<I>, std::vec::IntoIter<V>>;

    fn into_iter(self) -> Self::IntoIter {
        self.indices.into_iter().zip(self.dense)
    }
}

impl<I: Index, V> FromIterator<(I, V)> for SparseSet<I, V> {
    fn from_iter<T: IntoIterator<Item = (I, V)>>(iter: T) -> Self {
        let mut set = Self::new();

        for (index, value) in iter {
            set.insert(index, value);
        }

        set
    }
}

impl<I: Index> FromIterator<I> for SparseSet<I, ()> {
    fn from_iter<T: IntoIterator<Item = I>>(iter: T) -> Self {
        let mut set = Self::new();

        for index in iter {
            set.insert(index, ());
        }

        set
    }
}

impl<I: Index, V> Extend<(I, V)> for SparseSet<I, V> {
    fn extend<T: IntoIterator<Item = (I, V)>>(&mut self, iter: T) {
        for (index, value) in iter {
            self.insert(index, value);
        }
    }
}

impl<I: Index + Clone, V: Clone> Clone for SparseSet<I, V> {
    fn clone(&self) -> Self {
        Self {
            indices: self.indices.clone(),
            dense: self.dense.clone(),
            sparse: self.sparse.clone(),
        }
    }
}

/// A single "column" of an archetypal table.
/// Contains all the instances of a single component type within a single archetype.
pub struct Column {
    pub(crate) id: DynamicId,
    pub(crate) storage: SparseSet<DynamicId, Data>,
}

impl HasIndex for Column {
    type Index = DynamicId;

    fn index(&self) -> Self::Index {
        self.id
    }
}

impl Debug for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentStorage")
            .field("component_id", &self.id)
            .field("entities", &self.storage.sparse_iter().collect::<Vec<_>>())
            .finish()
    }
}

impl Column {
    pub fn new_with_static<T: Component>(registry: &Registry) -> Self {
        Self {
            id: registry.get_static::<T>(),
            storage: SparseSet::new(),
        }
    }

    pub fn new_with_id(info: DynamicId) -> Self {
        Self {
            id: info,
            storage: SparseSet::new(),
        }
    }

    pub fn insert(&mut self, entity: DynamicId, component: Data) {
        assert_eq!(self.id, component.type_id());
        self.storage.insert(entity, component);
    }

    pub fn remove(&mut self, entity: &DynamicId) -> Option<Data> {
        self.storage.remove(entity)
    }

    pub fn get(&self, entity: &DynamicId) -> Option<&Data> {
        self.storage.get(entity)
    }

    pub fn get_mut(&mut self, entity: &DynamicId) -> Option<&mut Data> {
        self.storage.get_mut(entity)
    }
}

/// A unique combination of components. Also maintains a list of entities that have this combination.
#[derive(Debug)]
pub struct Archetype {
    pub(crate) id: usize,
    pub(crate) entities: SparseSet<DynamicId, ()>,
    pub(crate) component_types: Box<[DynamicId]>,
    pub(crate) columns: SortedMap<DynamicId, Column>,
}

impl Archetype {
    pub fn with_component_ids(arch_id: usize, component_ids: Vec<DynamicId>) -> Self {
        let mut columns = Vec::new();

        for &id in component_ids.iter() {
            columns.push((id, Column::new_with_id(id)));
        }

        Self {
            id: arch_id,
            entities: SparseSet::default(),
            component_types: component_ids.into_boxed_slice(),
            columns: SortedMap::new(columns),
        }
    }

    pub fn insert_entity(&mut self, entity: DynamicId, component: Data) {
        self.entities.insert(entity, ());

        let component_id = component.type_id();

        self.columns
            .get_mut(&component_id)
            .unwrap()
            .insert(entity, component);
    }

    pub fn remove_entity(&mut self, entity: DynamicId) {
        self.entities.remove(&entity);

        for (_, column) in self.columns.iter_mut() {
            column.remove(&entity);
        }
    }

    pub fn exclusively_contains_components(&self, component_ids: &[DynamicId]) -> bool {
        component_ids
            .iter()
            .all(|index| self.columns.contains_key(index))
            && self.columns.len() == component_ids.len()
    }

    pub fn contains_entity(&self, entity: DynamicId) -> bool {
        self.entities.contains(&entity)
    }

    pub fn get_column(&self, component_id: DynamicId) -> Option<&Column> {
        self.columns.get(&component_id)
    }

    pub fn component_drain(&mut self, entity: DynamicId) -> impl Iterator<Item = Data> + '_ {
        self.entities.remove(&entity);
        self.columns
            .iter_mut()
            .filter_map(move |(_, column)| column.remove(&entity))
    }

    pub fn component_ids(&self) -> ComponentSet {
        self.columns.iter().map(|x| x.0).collect()
    }
}

pub struct Components {
    registry: Arc<Registry>,
    living_entities: SparseSet<DynamicId, Entity>,
    entity_generations: SparseSet<DynamicId, u32>,
    archetypes: Vec<Archetype>,
    entity_archetypes: SparseSet<DynamicId, usize>,
    component_archetypes: ComponentMap<ArchetypeSet>,
}

impl Components {
    pub fn new() -> Self {
        Self {
            registry: Registry::new(),
            living_entities: SparseSet::new(),
            entity_generations: SparseSet::new(),
            archetypes: Vec::new(),
            entity_archetypes: SparseSet::new(),
            component_archetypes: SparseSet::new(),
        }
    }

    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    pub fn create_entity(&mut self) -> Entity {
        let id = self.registry.create();
        let entity = Entity::new(id);

        self.entity_generations.insert(id, entity.generation());
        self.living_entities.insert(id, entity);

        entity
    }

    pub fn entity(&self, id: DynamicId) -> Option<Entity> {
        if self.living_entities.contains(&id) {
            Some(Entity::new_with_generation(
                id,
                *self.entity_generations.get(&id)?,
            ))
        } else {
            None
        }
    }

    pub fn living_entities(&self) -> impl Iterator<Item = &Entity> + '_ {
        self.living_entities.dense_iter()
    }

    pub fn add_component<T: Component>(
        &mut self,
        entity: &Entity,
        component: T,
        field_name: Option<&str>,
    ) {
        // the components of the entity are changing, therefore the archetype must change
        let components = if let Some(old_archetype) = self.entity_archetype_mut(entity.id()) {
            // add the component to create the new archetype
            let mut components = old_archetype
                .component_drain(entity.id())
                .collect::<Vec<_>>();
            components.push(Data::new(component, field_name, &self.registry));
            components
        } else {
            vec![Data::new(component, field_name, &self.registry)]
        };

        let component_ids = components.iter().map(|x| x.type_id()).collect::<Vec<_>>();

        self.build_on_with_components(entity.id(), components, component_ids);
    }

    pub fn add_dynamic_component(&mut self, entity: &Entity, component: Data) {
        // the components of the entity are changing, therefore the archetype must change
        let components = if let Some(old_archetype) = self.entity_archetype_mut(entity.id()) {
            // add the component to create the new archetype
            let mut components = old_archetype
                .component_drain(entity.id())
                .collect::<Vec<_>>();
            components.push(component);
            components
        } else {
            vec![component]
        };

        let component_ids = components.iter().map(|x| x.type_id()).collect::<Vec<_>>();

        self.build_on_with_components(entity.id(), components, component_ids);
    }

    pub fn build<B: Bundle>(&mut self, bundle: B) -> Entity {
        // add the components in bulk
        let components = bundle.components(&self.registry);
        let component_types = B::component_types(&self.registry);

        self.build_with_components(components, component_types)
    }

    pub fn build_with_components(
        &mut self,
        components: Vec<Data>,
        component_ids: Vec<DynamicId>,
    ) -> Entity {
        let entity = self.create_entity();
        self.build_on_with_components(entity.id(), components, component_ids);

        entity
    }

    pub(crate) fn build_on_with_components(
        &mut self,
        entity: DynamicId,
        mut components: Vec<Data>,
        component_ids: Vec<DynamicId>,
    ) {
        // find the archetype that matches the components, if any
        for archetype in self.archetypes.iter_mut() {
            if archetype.exclusively_contains_components(&component_ids) {
                for component in components.drain(..) {
                    archetype.insert_entity(entity, component);
                }
                self.entity_archetypes.insert(entity, archetype.id);
                return;
            }
        }

        // if we didn't find an archetype, create a new one
        let mut archetype = Archetype::with_component_ids(self.archetypes.len(), component_ids);

        if let Some(old_archetype) = self.entity_archetype_mut(entity) {
            for component in old_archetype.component_drain(entity) {
                archetype.insert_entity(entity, component);
            }
            old_archetype.remove_entity(entity);
        }

        for component in components {
            archetype.insert_entity(entity, component);
        }

        self.entity_archetypes.insert(entity, archetype.id);
        self.push_archetype(archetype);
    }

    fn push_archetype(&mut self, archetype: Archetype) {
        // todo: assert that the archetype is unique

        for component_id in archetype.component_ids().sparse_iter() {
            if let Some(archetype_set) = self.component_archetypes.get_mut(component_id) {
                archetype_set.insert(archetype.id, ());
            } else {
                let mut archetype_set = ArchetypeSet::default();
                archetype_set.insert(archetype.id, ());
                self.component_archetypes
                    .insert(*component_id, archetype_set);
            }
        }

        self.archetypes.push(archetype);
    }

    pub fn despawn(&mut self, entity: DynamicId) -> bool {
        let archetype = self.entity_archetype_mut(entity);
        if archetype.is_none() {
            return false;
        }
        let archetype = archetype.unwrap();
        archetype.remove_entity(entity);
        self.living_entities.remove(&entity);
        *self.entity_generations.get_mut(&entity).unwrap() += 1;
        self.entity_archetypes.remove(&entity);
        true
    }

    pub fn respawn(&mut self, entity: DynamicId) -> Entity {
        let e = Entity::new_with_generation(entity, *self.entity_generations.get(&entity).unwrap());
        if self.living_entities.contains(&entity) {
            e
        } else {
            self.living_entities.insert(entity, e);
            e
        }
    }

    pub fn entity_archetype(&self, entity: DynamicId) -> Option<&Archetype> {
        self.entity_archetypes
            .get(&entity)
            .and_then(|index| self.archetypes.get(*index))
    }

    pub fn entity_archetype_mut(&mut self, entity: DynamicId) -> Option<&mut Archetype> {
        self.entity_archetypes
            .get(&entity)
            .and_then(|index| self.archetypes.get_mut(*index))
    }

    pub fn component_archetypes(&self, component_id: DynamicId) -> Option<&ArchetypeSet> {
        self.component_archetypes.get(&component_id)
    }

    pub fn component_archetypes_mut(
        &mut self,
        component_id: DynamicId,
    ) -> Option<&mut ArchetypeSet> {
        self.component_archetypes.get_mut(&component_id)
    }

    pub fn entity_components(&self, entity: DynamicId) -> Option<Vec<&Data>> {
        self.entity_archetype(entity).map(|archetype| {
            archetype
                .columns
                .iter()
                .filter_map(|(_, column)| column.get(&entity))
                .collect()
        })
    }

    pub fn entity_components_iter(&self, entity: DynamicId) -> Option<impl Iterator<Item = &Data>> {
        self.entity_archetype(entity).map(|archetype| {
            archetype
                .columns
                .iter()
                .filter_map(move |(_, column)| column.get(&entity))
        })
    }

    pub fn entity_components_iter_mut(
        &mut self,
        entity: DynamicId,
    ) -> Option<impl Iterator<Item = &mut Data>> {
        self.entity_archetype_mut(entity).map(|archetype| {
            archetype
                .columns
                .iter_mut()
                .filter_map(move |(_, column)| column.get_mut(&entity))
        })
    }

    pub fn split(&self) -> Components {
        let mut components = Components::new();
        let registry = self.registry.split();

        components.registry = Arc::new(registry);

        components
    }

    pub fn merge(&mut self, mut other: Components) {
        self.registry.merge(&other.registry);

        for (id, entity) in other
            .living_entities
            .iter()
            .map(|(id, entity)| (*id, *entity))
            .collect::<Vec<_>>()
        {
            if !self.living_entities.contains(&id) {
                self.living_entities.insert(id, entity);
                self.entity_generations.insert(id, entity.generation());

                let archetype = other.entity_archetype_mut(id).unwrap();

                let components = archetype.component_drain(id).collect::<Vec<_>>();

                let component_types = archetype.component_types.to_vec();

                self.build_on_with_components(id, components, component_types);
            }
        }
    }

    pub fn entities_matching_access<'a>(
        &'a self,
        access: &'a QueryAccess,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.archetypes
            .iter()
            .flat_map(move |archetype| {
                if access.matches_archetype(archetype) {
                    Some(
                        archetype
                            .entities
                            .sparse_iter()
                            .flat_map(|entity| self.entity(*entity)),
                    )
                } else {
                    None
                }
            })
            .flatten()
    }

    pub fn components_matching_access(
        &self,
        access: &QueryAccess,
    ) -> SparseSet<Entity, ComponentMap<Data>> {
        self.archetypes
            .iter()
            .flat_map(move |archetype| {
                if access.matches_archetype(archetype) {
                    Some(
                        archetype
                            .entities
                            .sparse_iter()
                            .flat_map(|entity| self.entity(*entity))
                            .map(move |entity| {
                                (
                                    entity,
                                    archetype
                                        .columns
                                        .iter()
                                        .filter_map(move |(id, column)| {
                                            column
                                                .storage
                                                .get(&entity.id())
                                                .map(|x| (id, x.to_owned()))
                                        })
                                        .collect(),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                }
            })
            .flatten()
            .collect()
    }
}

impl Default for Components {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TemporaryComponents {
    pub components: Components,
}
