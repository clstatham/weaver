use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    ops::Deref,
};

use weaver_util::lock::{ArcRead, ArcWrite, SharedLock};

use crate::prelude::Bundle;

use super::{component::Component, entity::Entity};

pub struct Data {
    pub type_id: TypeId,
    pub data: Box<dyn Component>,
}

impl Data {
    pub fn new<T: Component>(data: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            data: Box::new(data),
        }
    }

    pub fn new_dynamic(data: Box<dyn Component>) -> Self {
        Self {
            type_id: (*data).as_any().type_id(),
            data,
        }
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub fn is<T: Component>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    pub fn get_data(&self) -> &dyn Component {
        &*self.data
    }

    pub fn get_data_mut(&mut self) -> &mut dyn Component {
        &mut *self.data
    }

    pub fn set_data(&mut self, data: Box<dyn Component>) {
        self.data = data;
    }

    pub fn into_data(self) -> Box<dyn Component> {
        self.data
    }

    pub fn downcast_ref<T: Component>(&self) -> Option<&T> {
        (*self.data).downcast_ref()
    }

    pub fn downcast_mut<T: Component>(&mut self) -> Option<&mut T> {
        (*self.data).downcast_mut()
    }
}

pub struct SparseSet<T> {
    pub(crate) dense: Vec<T>,
    sparse: Vec<Option<usize>>,
    indices: Vec<usize>,
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self {
            dense: Vec::new(),
            sparse: Vec::new(),
            indices: Vec::new(),
        }
    }
}

impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dense_index_of(&self, id: usize) -> Option<usize> {
        self.sparse.get(id).copied().flatten()
    }

    pub fn insert(&mut self, id: usize, data: T) {
        match self.dense_index_of(id) {
            Some(index) => {
                self.dense[index] = data;
            }
            None => {
                let index = self.dense.len();
                self.dense.push(data);
                if id >= self.sparse.len() {
                    self.sparse.resize(id + 1, None);
                }
                self.sparse[id] = Some(index);
                self.indices.push(id);
            }
        }
    }

    pub fn remove(&mut self, id: usize) -> Option<T> {
        if id >= self.sparse.len() {
            return None;
        }

        let index = self.sparse[id].take()?;

        let value = self.dense.swap_remove(index);
        let _ = self.indices.swap_remove(index);

        if index < self.dense.len() {
            let swapped = self.indices[index];
            self.sparse[swapped] = Some(index);
        }

        Some(value)
    }

    pub fn get(&self, id: usize) -> Option<&T> {
        if id >= self.sparse.len() {
            return None;
        }

        let dense_index = self.sparse[id]?;
        self.dense.get(dense_index)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut T> {
        if id >= self.sparse.len() {
            return None;
        }

        let dense_index = self.sparse[id]?;
        self.dense.get_mut(dense_index)
    }

    pub fn contains(&self, id: usize) -> bool {
        self.sparse.get(id).copied().flatten().is_some()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.dense.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.dense.iter_mut()
    }

    pub fn sparse_iter(&self) -> std::slice::Iter<'_, usize> {
        self.indices.iter()
    }

    pub fn sparse_index_of(&self, index: usize) -> Option<usize> {
        self.indices.get(index).copied()
    }

    pub fn clear(&mut self) {
        self.dense.clear();
        self.sparse.clear();
        self.indices.clear();
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }
}

#[derive(Clone)]
pub struct ColumnRef {
    column: SharedLock<SparseSet<Data>>,
}

impl ColumnRef {
    pub fn new(column: SharedLock<SparseSet<Data>>) -> Self {
        Self { column }
    }
}

impl Deref for ColumnRef {
    type Target = SharedLock<SparseSet<Data>>;

    fn deref(&self) -> &Self::Target {
        &self.column
    }
}

#[derive(Default)]
pub struct Archetype {
    columns: HashMap<TypeId, SharedLock<SparseSet<Data>>>,
    entities: HashSet<Entity>,
}

impl Archetype {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_ids(&self) -> Vec<TypeId> {
        self.columns.keys().copied().collect::<Vec<_>>()
    }

    pub fn insert(&mut self, entity: Entity, data: Data) {
        let type_id = data.type_id();

        self.entities.insert(entity);

        self.columns[&type_id]
            .write_arc()
            .insert(entity.as_usize(), data);
    }

    pub fn remove(&mut self, entity: Entity) -> Vec<Data> {
        self.entities.remove(&entity);

        self.columns
            .values()
            .filter_map(|column| column.write_arc().remove(entity.as_usize()))
            .collect()
    }

    #[inline]
    pub fn get<T: Component>(&self, entity: Entity) -> Option<DataRef> {
        self.get_by_type_id(entity, TypeId::of::<T>())
    }

    #[inline]
    pub fn get_mut<T: Component>(&self, entity: Entity) -> Option<DataMut> {
        self.get_by_type_id_mut(entity, TypeId::of::<T>())
    }

    pub fn get_by_type_id(&self, entity: Entity, type_id: TypeId) -> Option<DataRef> {
        if self.columns[&type_id]
            .read_arc()
            .contains(entity.as_usize())
        {
            Some(DataRef::new(entity, self.columns[&type_id].read_arc()))
        } else {
            None
        }
    }

    pub fn get_by_type_id_mut(&self, entity: Entity, type_id: TypeId) -> Option<DataMut> {
        if self.columns[&type_id]
            .read_arc()
            .contains(entity.as_usize())
        {
            Some(DataMut::new(entity, self.columns[&type_id].write_arc()))
        } else {
            None
        }
    }

    #[inline]
    pub fn get_column<T: Component>(&self) -> Option<ColumnRef> {
        self.get_column_by_type_id(TypeId::of::<T>())
    }

    pub fn get_column_by_type_id(&self, type_id: TypeId) -> Option<ColumnRef> {
        Some(ColumnRef::new(self.columns[&type_id].clone()))
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        self.has_component_by_type_id(entity, TypeId::of::<T>())
    }

    pub fn has_component_by_type_id(&self, entity: Entity, type_id: TypeId) -> bool {
        self.columns
            .get(&type_id)
            .map(|c| c.read_arc().contains(entity.as_usize()))
            .unwrap_or(false)
    }

    pub fn contains_component_by_type_id(&self, type_id: TypeId) -> bool {
        self.columns.contains_key(&type_id)
    }

    pub fn contains_entity(&self, entity: Entity) -> bool {
        self.columns
            .values()
            .any(|column| column.read_arc().contains(entity.as_usize()))
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn contains_all_types(&self, type_ids: &[TypeId]) -> bool {
        type_ids
            .iter()
            .all(|type_id| self.columns.contains_key(type_id))
    }

    pub fn contains_any_type(&self, type_ids: &[TypeId]) -> bool {
        type_ids
            .iter()
            .any(|type_id| self.columns.contains_key(type_id))
    }

    pub fn exclusively_contains_types(&self, type_ids: &[TypeId]) -> bool {
        type_ids
            .iter()
            .all(|type_id| self.columns.contains_key(type_id))
            && self.columns.len() == type_ids.len()
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter().copied()
    }
}

pub struct DataRef {
    entity: Entity,
    column: ArcRead<SparseSet<Data>>,
}

impl DataRef {
    pub fn new(entity: Entity, column: ArcRead<SparseSet<Data>>) -> Self {
        Self { entity, column }
    }

    pub fn entity(this: &Self) -> Entity {
        this.entity
    }
}

impl std::ops::Deref for DataRef {
    type Target = Data;

    fn deref(&self) -> &Self::Target {
        self.column.get(self.entity.as_usize()).unwrap()
    }
}

pub struct DataMut {
    entity: Entity,
    column: ArcWrite<SparseSet<Data>>,
}

impl DataMut {
    pub fn new(entity: Entity, column: ArcWrite<SparseSet<Data>>) -> Self {
        Self { entity, column }
    }

    pub fn entity(this: &Self) -> Entity {
        this.entity
    }
}

impl std::ops::Deref for DataMut {
    type Target = Data;

    fn deref(&self) -> &Self::Target {
        self.column.get(self.entity.as_usize()).unwrap()
    }
}

impl std::ops::DerefMut for DataMut {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.column.get_mut(self.entity.as_usize()).unwrap()
    }
}

pub struct Ref<T: Component> {
    dense_index: usize,
    column: ArcRead<SparseSet<Data>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Component> Ref<T> {
    pub fn new(dense_index: usize, column: ArcRead<SparseSet<Data>>) -> Self {
        Self {
            dense_index,
            column,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: Component> std::ops::Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.column.dense[self.dense_index].downcast_ref().unwrap()
    }
}

pub struct Mut<T: Component> {
    dense_index: usize,
    column: ArcWrite<SparseSet<Data>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Component> Mut<T> {
    pub fn new(dense_index: usize, column: ArcWrite<SparseSet<Data>>) -> Self {
        Self {
            dense_index,
            column,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: Component> std::ops::Deref for Mut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.column.dense[self.dense_index].downcast_ref().unwrap()
    }
}

impl<T: Component> std::ops::DerefMut for Mut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.column.dense[self.dense_index].downcast_mut().unwrap()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArchetypeId(usize);

#[derive(Default)]
pub struct Storage {
    next_archetype_id: usize,
    archetypes: HashMap<ArchetypeId, Archetype>,
    entity_archetype: HashMap<Entity, ArchetypeId>,
}

impl Storage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_components<T: Bundle>(&mut self, entity: Entity, bundle: T) {
        let old_archetype_id = self.entity_archetype.remove(&entity);
        let old_archetype = old_archetype_id.and_then(|id| self.archetypes.get_mut(&id));

        let components = bundle.into_components();
        let mut data = components
            .into_iter()
            .map(Data::new_dynamic)
            .collect::<Vec<_>>();

        if let Some(old_archetype) = old_archetype {
            let mut old_data = old_archetype.remove(entity);

            data.append(&mut old_data);
        }

        let existing = self
            .archetypes
            .iter_mut()
            .find(|(_, archetype)| {
                archetype.exclusively_contains_types(
                    &data
                        .iter()
                        .map(|component| component.type_id())
                        .collect::<Vec<_>>(),
                )
            })
            .map(|(id, archetype)| (archetype, *id));

        let (archetype, archetype_id) = if let Some((archetype, existing_archetype_id)) = existing {
            (archetype, existing_archetype_id)
        } else {
            let archetype_id = ArchetypeId(self.next_archetype_id);
            self.next_archetype_id += 1;

            let mut archetype = Archetype::new();

            archetype.columns = data
                .iter()
                .map(|data| (data.type_id(), SharedLock::new(SparseSet::new())))
                .collect();

            self.archetypes.insert(archetype_id, archetype);

            let archetype = self.archetypes.get_mut(&archetype_id).unwrap();

            (archetype, archetype_id)
        };

        for data in data {
            archetype.insert(entity, data);
        }

        self.entity_archetype.insert(entity, archetype_id);

        if let Some(old_archetype_id) = old_archetype_id {
            if self.archetypes[&old_archetype_id].is_empty() {
                self.archetypes.remove(&old_archetype_id);
            }
        }
    }

    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) {
        self.insert_components(entity, component);
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        // remove the entity from its current archetype
        let old_archetype_id = self.entity_archetype.remove(&entity)?;
        let old_archetype = self.archetypes.get_mut(&old_archetype_id)?;

        let mut data = old_archetype.remove(entity);

        if old_archetype.is_empty() {
            self.archetypes.remove(&old_archetype_id);
        }

        self.entity_archetype.remove(&entity);

        // remove the component from the data
        let component = data.remove(data.iter().position(|data| data.is::<T>())?);

        // find the new archetype for the entity
        let new_archetype = self
            .archetypes
            .iter_mut()
            .find(|(_, archetype)| {
                archetype.exclusively_contains_types(
                    &data.iter().map(|data| data.type_id()).collect::<Vec<_>>(),
                )
            })
            .map(|(id, archetype)| (archetype, *id));

        let (new_archetype, new_archetype_id) =
            if let Some((archetype, existing_archetype_id)) = new_archetype {
                (archetype, existing_archetype_id)
            } else {
                let archetype_id = ArchetypeId(self.next_archetype_id);
                self.next_archetype_id += 1;

                let mut archetype = Archetype::new();

                archetype.columns = data
                    .iter()
                    .map(|data| (data.type_id(), SharedLock::new(SparseSet::new())))
                    .collect();

                self.archetypes.insert(archetype_id, archetype);

                let archetype = self.archetypes.get_mut(&archetype_id).unwrap();

                (archetype, archetype_id)
            };

        for data in data {
            new_archetype.insert(entity, data);
        }

        self.entity_archetype.insert(entity, new_archetype_id);

        let Ok(component) = component.into_data().downcast::<T>() else {
            panic!("downcast failed: expected {}", std::any::type_name::<T>());
        };

        Some(*component)
    }

    pub fn remove_entity(&mut self, entity: Entity) -> Option<Vec<Data>> {
        let archetype_id = self.entity_archetype.get(&entity)?;

        let archetype = self.archetypes.get_mut(archetype_id)?;

        let data = archetype.remove(entity);

        if archetype.is_empty() {
            self.archetypes.remove(archetype_id);
        }

        self.entity_archetype.remove(&entity);

        Some(data)
    }

    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<Ref<T>> {
        let archetype_id = self.entity_archetype.get(&entity)?;

        let archetype = self.archetypes.get(archetype_id)?;

        if archetype
            .columns
            .get(&TypeId::of::<T>())?
            .read_arc()
            .contains(entity.as_usize())
        {
            let column = archetype.columns[&TypeId::of::<T>()].read_arc();
            let dense_index = column.dense_index_of(entity.as_usize()).unwrap();
            Some(Ref::new(dense_index, column))
        } else {
            None
        }
    }

    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<Mut<T>> {
        let archetype_id = self.entity_archetype.get(&entity)?;

        let archetype = self.archetypes.get(archetype_id)?;

        if archetype
            .columns
            .get(&TypeId::of::<T>())?
            .read_arc()
            .contains(entity.as_usize())
        {
            let column = archetype.columns[&TypeId::of::<T>()].write_arc();
            let dense_index = column.dense_index_of(entity.as_usize()).unwrap();
            Some(Mut::new(dense_index, column))
        } else {
            None
        }
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        self.has_component_by_type_id(entity, TypeId::of::<T>())
    }

    pub fn has_component_by_type_id(&self, entity: Entity, type_id: TypeId) -> bool {
        if let Some(archetype_id) = self.entity_archetype.get(&entity) {
            if let Some(archetype) = self.archetypes.get(archetype_id) {
                return archetype.has_component_by_type_id(entity, type_id);
            }
        }

        false
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entity_archetype.keys().copied()
    }

    pub fn archetype_iter(&self) -> impl Iterator<Item = &Archetype> + '_ {
        self.archetypes.values()
    }

    pub fn get_archetype(&self, entity: Entity) -> Option<&Archetype> {
        self.entity_archetype
            .get(&entity)
            .and_then(|archetype_id| self.archetypes.get(archetype_id))
    }
}

#[cfg(test)]
mod tests {
    use weaver_ecs_macros::Component;

    use super::*;
    use crate as weaver_ecs;

    #[derive(Debug, PartialEq, Clone, Component)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, PartialEq, Clone, Component)]
    struct Velocity {
        dx: f32,
        dy: f32,
    }

    #[derive(Debug, PartialEq, Clone, Component)]
    struct Acceleration {
        ddx: f32,
        ddy: f32,
    }

    #[test]
    fn test_insert_component() {
        let mut storage = Storage::new();

        let entity = Entity::new(0, 0);

        storage.insert_component(entity, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity, Velocity { dx: 1.0, dy: 1.0 });

        assert_eq!(
            storage.get_component::<Position>(entity).map(|data| data.x),
            Some(0.0)
        );
        assert_eq!(
            storage
                .get_component::<Velocity>(entity)
                .map(|data| data.dx),
            Some(1.0)
        );
    }

    #[test]
    fn test_remove_component() {
        let mut storage = Storage::new();

        let entity = Entity::new(0, 0);

        storage.insert_component(entity, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity, Velocity { dx: 1.0, dy: 1.0 });

        assert_eq!(
            storage.get_component::<Position>(entity).map(|data| data.x),
            Some(0.0)
        );
        assert_eq!(
            storage
                .get_component::<Velocity>(entity)
                .map(|data| data.dx),
            Some(1.0)
        );

        storage.remove_component::<Position>(entity);

        assert_eq!(storage.get_component::<Position>(entity).as_deref(), None);
        assert_eq!(
            storage
                .get_component::<Velocity>(entity)
                .map(|data| data.dx),
            Some(1.0)
        );
    }

    #[test]
    fn test_remove_entity() {
        let mut storage = Storage::new();

        let entity = Entity::new(0, 0);

        storage.insert_component(entity, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity, Velocity { dx: 1.0, dy: 1.0 });

        assert_eq!(
            storage.get_component::<Position>(entity).map(|data| data.x),
            Some(0.0)
        );
        assert_eq!(
            storage
                .get_component::<Velocity>(entity)
                .map(|data| data.dx),
            Some(1.0)
        );

        let data = storage.remove_entity(entity).unwrap();

        assert_eq!(data.len(), 2);

        assert_eq!(storage.get_component::<Position>(entity).as_deref(), None);
        assert_eq!(storage.get_component::<Velocity>(entity).as_deref(), None);
    }

    #[test]
    fn test_get() {
        let mut storage = Storage::new();

        let entity = Entity::new(0, 0);

        storage.insert_component(entity, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity, Velocity { dx: 1.0, dy: 1.0 });

        assert_eq!(
            storage.get_component::<Position>(entity).map(|data| data.x),
            Some(0.0)
        );
        assert_eq!(
            storage
                .get_component::<Velocity>(entity)
                .map(|data| data.dx),
            Some(1.0)
        );
        assert_eq!(
            storage.get_component::<Acceleration>(entity).as_deref(),
            None
        );
    }

    #[test]
    fn test_contains() {
        let mut storage = Storage::new();

        let entity = Entity::new(0, 0);

        storage.insert_component(entity, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity, Velocity { dx: 1.0, dy: 1.0 });

        assert!(storage.has_component::<Position>(entity));
        assert!(storage.has_component::<Velocity>(entity));
        assert!(!storage.has_component::<Acceleration>(entity));
    }

    #[test]
    fn test_entity_iter() {
        let mut storage = Storage::new();

        let entity1 = Entity::new(0, 0);
        let entity2 = Entity::new(1, 0);
        let entity3 = Entity::new(2, 0);

        storage.insert_component(entity1, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity2, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity3, Position { x: 0.0, y: 0.0 });

        let entities = storage.entity_iter().collect::<Vec<_>>();

        assert_eq!(entities.len(), 3);
        assert!(entities.contains(&entity1));
        assert!(entities.contains(&entity2));
        assert!(entities.contains(&entity3));
    }

    #[test]
    fn test_archetype_iter() {
        let mut storage = Storage::new();

        let entity1 = Entity::new(0, 0);
        let entity2 = Entity::new(1, 0);
        let entity3 = Entity::new(2, 0);

        storage.insert_component(entity1, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity2, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity3, Position { x: 0.0, y: 0.0 });

        let archetypes = storage.archetype_iter().collect::<Vec<_>>();

        assert_eq!(archetypes.len(), 1);
        assert_eq!(archetypes[0].len(), 3);
    }

    #[test]
    fn test_get_archetype() {
        let mut storage = Storage::new();

        let entity = Entity::new(0, 0);

        storage.insert_component(entity, Position { x: 0.0, y: 0.0 });
        storage.insert_component(entity, Velocity { dx: 1.0, dy: 1.0 });

        let archetype = storage.get_archetype(entity).unwrap();

        assert_eq!(archetype.len(), 1);
    }
}
