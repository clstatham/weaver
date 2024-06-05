use std::{any::TypeId, borrow::Cow, collections::HashMap};

use weaver_util::lock::{ArcRead, ArcWrite, SharedLock};

use super::{component::Component, entity::Entity};

pub struct Data {
    type_id: std::any::TypeId,
    type_name: Cow<'static, str>,
    data: Box<dyn Component>,
}

impl Data {
    pub fn new<T: Component>(data: T) -> Self {
        Self {
            type_id: std::any::TypeId::of::<T>(),
            type_name: std::any::type_name::<T>().into(),
            data: Box::new(data),
        }
    }

    pub fn type_id(&self) -> std::any::TypeId {
        self.type_id
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn is<T: Component>(&self) -> bool {
        self.type_id == std::any::TypeId::of::<T>()
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
        (*self.data).as_any().downcast_ref()
    }

    pub fn downcast_mut<T: Component>(&mut self) -> Option<&mut T> {
        (*self.data).as_any_mut().downcast_mut()
    }
}

#[derive(Default)]
pub struct SparseSet {
    dense: Vec<Data>,
    sparse: Vec<Option<usize>>,
    indices: Vec<usize>,
}

impl SparseSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dense_index_of(&self, id: usize) -> Option<usize> {
        self.sparse.get(id).copied().flatten()
    }

    pub fn insert(&mut self, id: usize, data: Data) {
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

    pub fn remove(&mut self, id: usize) -> Option<Data> {
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

    pub fn get(&self, id: usize) -> Option<&Data> {
        if id >= self.sparse.len() {
            return None;
        }

        let dense_index = self.sparse[id]?;
        self.dense.get(dense_index)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut Data> {
        if id >= self.sparse.len() {
            return None;
        }

        let dense_index = self.sparse[id]?;
        self.dense.get_mut(dense_index)
    }

    pub fn contains(&self, id: usize) -> bool {
        self.sparse.get(id).copied().flatten().is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Data> + '_ {
        self.dense.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Data> + '_ {
        self.dense.iter_mut()
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

#[derive(Default)]
pub struct Archetype {
    columns: Vec<SharedLock<SparseSet>>,
    type_ids: Vec<std::any::TypeId>,
    entities: Vec<Entity>,
}

impl Archetype {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entity: Entity, data: Data) {
        let type_id = data.type_id();

        let index = self
            .type_ids
            .iter()
            .position(|&id| id == type_id)
            .unwrap_or_else(|| {
                self.type_ids.push(type_id);
                self.columns.push(SharedLock::new(SparseSet::new()));
                self.columns.len() - 1
            });

        self.entities.push(entity);

        self.columns[index].write().insert(entity.as_usize(), data)
    }

    pub fn remove(&mut self, entity: Entity) -> Vec<Data> {
        self.entities.retain(|e| *e != entity);

        self.columns
            .iter_mut()
            .filter_map(|column| column.write().remove(entity.as_usize()))
            .collect()
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<DataRef> {
        let type_id = std::any::TypeId::of::<T>();

        let index = self.type_ids.iter().position(|&id| id == type_id)?;

        if self.columns[index].read().contains(entity.as_usize()) {
            Some(DataRef::new(entity, self.columns[index].read()))
        } else {
            None
        }
    }

    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<DataMut> {
        let type_id = std::any::TypeId::of::<T>();

        let index = self.type_ids.iter().position(|&id| id == type_id)?;

        if self.columns[index].read().contains(entity.as_usize()) {
            Some(DataMut::new(entity, self.columns[index].write()))
        } else {
            None
        }
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        let type_id = std::any::TypeId::of::<T>();

        self.type_ids
            .iter()
            .position(|&id| id == type_id)
            .map(|index| self.columns[index].read().contains(entity.as_usize()))
            .unwrap_or(false)
    }

    pub fn has_component_by_type_id(&self, type_id: std::any::TypeId) -> bool {
        self.type_ids.contains(&type_id)
    }

    pub fn contains_entity(&self, entity: Entity) -> bool {
        self.columns
            .iter()
            .any(|column| column.read().contains(entity.as_usize()))
    }

    pub fn clear(&mut self) {
        self.columns.clear();
        self.type_ids.clear();
    }

    pub fn len(&self) -> usize {
        self.columns.iter().map(|column| column.read().len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.iter().all(|column| column.read().is_empty())
    }

    pub fn exclusively_contains_types(&self, type_ids: &[std::any::TypeId]) -> bool {
        type_ids
            .iter()
            .all(|type_id| self.type_ids.contains(type_id))
            && self.type_ids.len() == type_ids.len()
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter().copied()
    }
}

pub struct DataRef {
    entity: Entity,
    column: ArcRead<SparseSet>,
}

impl DataRef {
    pub fn new(entity: Entity, column: ArcRead<SparseSet>) -> Self {
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
    column: ArcWrite<SparseSet>,
}

impl DataMut {
    pub fn new(entity: Entity, column: ArcWrite<SparseSet>) -> Self {
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

pub struct Ref<T: 'static> {
    entity: Entity,
    column: ArcRead<SparseSet>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: 'static> Ref<T> {
    pub fn new(entity: Entity, column: ArcRead<SparseSet>) -> Self {
        Self {
            entity,
            column,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn entity(this: &Self) -> Entity {
        this.entity
    }
}

impl<T: 'static> std::ops::Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.column
            .get(self.entity.as_usize())
            .unwrap()
            .downcast_ref()
            .unwrap()
    }
}

pub struct Mut<T: 'static> {
    entity: Entity,
    column: ArcWrite<SparseSet>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: 'static> Mut<T> {
    pub fn new(entity: Entity, column: ArcWrite<SparseSet>) -> Self {
        Self {
            entity,
            column,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn entity(this: &Self) -> Entity {
        this.entity
    }
}

impl<T: 'static> std::ops::Deref for Mut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.column
            .get(self.entity.as_usize())
            .unwrap()
            .downcast_ref()
            .unwrap()
    }
}

impl<T: 'static> std::ops::DerefMut for Mut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.column
            .get_mut(self.entity.as_usize())
            .unwrap()
            .downcast_mut()
            .unwrap()
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

    pub fn insert_component<T: Component>(&mut self, entity: Entity, new_data: T) {
        let old_archetype_id = self.entity_archetype.remove(&entity);
        let old_archetype = old_archetype_id.and_then(|id| self.archetypes.get_mut(&id));

        let data = if let Some(old_archetype) = old_archetype {
            let mut data = old_archetype.remove(entity);
            data.push(Data::new(new_data));
            data
        } else {
            vec![Data::new(new_data)]
        };

        let existing = self
            .archetypes
            .iter_mut()
            .find(|(_, archetype)| {
                archetype.exclusively_contains_types(
                    &data.iter().map(|data| data.type_id()).collect::<Vec<_>>(),
                )
            })
            .map(|(id, archetype)| (archetype, *id));

        let (archetype, archetype_id) = if let Some((archetype, existing_archetype_id)) = existing {
            (archetype, existing_archetype_id)
        } else {
            let archetype_id = ArchetypeId(self.next_archetype_id);
            self.next_archetype_id += 1;

            let archetype = Archetype::new();

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

    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        // remove the entity from its current archetype

        let old_archetype_id = self.entity_archetype.get(&entity)?;

        let old_archetype = self.archetypes.get_mut(old_archetype_id)?;

        let mut data = old_archetype.remove(entity);

        if old_archetype.is_empty() {
            self.archetypes.remove(old_archetype_id);
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

                let archetype = Archetype::new();

                self.archetypes.insert(archetype_id, archetype);

                let archetype = self.archetypes.get_mut(&archetype_id).unwrap();

                (archetype, archetype_id)
            };

        for data in data {
            new_archetype.insert(entity, data);
        }

        self.entity_archetype.insert(entity, new_archetype_id);

        Some(*component.into_data().as_any_box().downcast::<T>().unwrap())
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

        let column_index = archetype
            .type_ids
            .iter()
            .position(|id| *id == std::any::TypeId::of::<T>())?;

        if archetype.columns[column_index]
            .read()
            .contains(entity.id() as usize)
        {
            Some(Ref::new(entity, archetype.columns[column_index].read()))
        } else {
            None
        }
    }

    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<Mut<T>> {
        let archetype_id = self.entity_archetype.get(&entity)?;

        let archetype = self.archetypes.get(archetype_id)?;

        let column_index = archetype
            .type_ids
            .iter()
            .position(|id| *id == std::any::TypeId::of::<T>())?;

        if archetype.columns[column_index]
            .read()
            .contains(entity.id() as usize)
        {
            Some(Mut::new(entity, archetype.columns[column_index].write()))
        } else {
            None
        }
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        if let Some(archetype_id) = self.entity_archetype.get(&entity) {
            if let Some(archetype) = self.archetypes.get(archetype_id) {
                return archetype.has_component::<T>(entity);
            }
        }

        false
    }

    pub fn has_component_by_type_id(&self, entity: Entity, type_id: TypeId) -> bool {
        if let Some(archetype_id) = self.entity_archetype.get(&entity) {
            if let Some(archetype) = self.archetypes.get(archetype_id) {
                return archetype.has_component_by_type_id(type_id);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Clone)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, PartialEq, Clone)]
    struct Velocity {
        dx: f32,
        dy: f32,
    }

    #[derive(Debug, PartialEq, Clone)]
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
        assert_eq!(
            data[0].downcast_ref::<Position>(),
            Some(&Position { x: 0.0, y: 0.0 })
        );
        assert_eq!(
            data[1].downcast_ref::<Velocity>(),
            Some(&Velocity { dx: 1.0, dy: 1.0 })
        );

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
}
