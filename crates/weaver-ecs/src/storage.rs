use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    ops::Deref,
};

use weaver_util::lock::{ArcRead, ArcWrite, SharedLock};

use crate::prelude::{
    Bundle, ChangeDetection, ChangeDetectionMut, ComponentTicks, Tick, Ticks, TicksMut,
};

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
    pub(crate) dense_added_ticks: Vec<SharedLock<Tick>>,
    pub(crate) dense_changed_ticks: Vec<SharedLock<Tick>>,
    sparse: Vec<Option<usize>>,
    indices: Vec<usize>,
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self {
            dense: Vec::new(),
            dense_added_ticks: Vec::new(),
            dense_changed_ticks: Vec::new(),
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

    pub fn insert(&mut self, id: usize, data: T, ticks: ComponentTicks) {
        match self.dense_index_of(id) {
            Some(index) => {
                self.dense[index] = data;
                *self.dense_added_ticks[index].write() = ticks.added;
                *self.dense_changed_ticks[index].write() = ticks.changed;
            }
            None => {
                let index = self.dense.len();

                self.dense.push(data);
                self.dense_added_ticks.push(SharedLock::new(ticks.added));
                self.dense_changed_ticks
                    .push(SharedLock::new(ticks.changed));

                if id >= self.sparse.len() {
                    self.sparse.resize(id + 1, None);
                }
                self.sparse[id] = Some(index);
                self.indices.push(id);
            }
        }
    }

    pub fn remove(&mut self, id: usize) -> Option<(T, ComponentTicks)> {
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

        Some((
            value,
            ComponentTicks {
                added: *self.dense_added_ticks[index].read(),
                changed: *self.dense_changed_ticks[index].read(),
            },
        ))
    }

    pub fn get(&self, id: usize) -> Option<&T> {
        if id >= self.sparse.len() {
            return None;
        }

        let dense_index = self.sparse[id]?;
        self.dense.get(dense_index)
    }

    pub fn get_mut(&mut self, id: usize, change_tick: Tick) -> Option<&mut T> {
        if id >= self.sparse.len() {
            return None;
        }

        let dense_index = self.sparse[id]?;

        *self.dense_changed_ticks[dense_index].write() = change_tick;

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

    pub fn get_ticks(&self, id: usize) -> Option<ComponentTicks> {
        if id >= self.sparse.len() {
            return None;
        }

        let dense_index = self.sparse[id]?;
        Some(ComponentTicks {
            added: *self.dense_added_ticks[dense_index].read(),
            changed: *self.dense_changed_ticks[dense_index].read(),
        })
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

    pub fn insert(&mut self, entity: Entity, data: Data, ticks: ComponentTicks) {
        let type_id = data.type_id();

        self.entities.insert(entity);

        self.columns[&type_id]
            .write_arc()
            .insert(entity.as_usize(), data, ticks);
    }

    pub fn remove(&mut self, entity: Entity) -> Vec<(Data, ComponentTicks)> {
        self.entities.remove(&entity);

        self.columns
            .values()
            .filter_map(|column| column.write_arc().remove(entity.as_usize()))
            .collect()
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

    pub fn column_iter(&self) -> impl Iterator<Item = (&TypeId, ColumnRef)> + '_ {
        self.columns
            .iter()
            .map(|(type_id, column)| (type_id, ColumnRef::new(column.clone())))
    }
}
pub struct Ref<T: Component> {
    dense_index: usize,
    column: ArcRead<SparseSet<Data>>,
    ticks: Ticks,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Component> Ref<T> {
    pub(crate) fn new(dense_index: usize, column: ArcRead<SparseSet<Data>>, ticks: Ticks) -> Self {
        Self {
            dense_index,
            column,
            ticks,
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

impl<T: Component> ChangeDetection for Ref<T> {
    fn is_added(&self) -> bool {
        self.ticks
            .added
            .is_newer_than(self.ticks.last_run, self.ticks.this_run)
    }

    fn is_changed(&self) -> bool {
        self.ticks
            .changed
            .is_newer_than(self.ticks.last_run, self.ticks.this_run)
    }

    fn last_changed(&self) -> Tick {
        *self.ticks.changed
    }
}

pub struct Mut<T: Component> {
    dense_index: usize,
    column: ArcWrite<SparseSet<Data>>,
    ticks: TicksMut,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Component> Mut<T> {
    pub(crate) fn new(
        dense_index: usize,
        column: ArcWrite<SparseSet<Data>>,
        ticks: TicksMut,
    ) -> Self {
        Self {
            dense_index,
            column,
            ticks,
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
        self.set_changed();
        self.column.dense[self.dense_index].downcast_mut().unwrap()
    }
}

impl<T: Component> ChangeDetection for Mut<T> {
    fn is_added(&self) -> bool {
        self.ticks
            .added
            .is_newer_than(self.ticks.last_run, self.ticks.this_run)
    }

    fn is_changed(&self) -> bool {
        self.ticks
            .changed
            .is_newer_than(self.ticks.last_run, self.ticks.this_run)
    }

    fn last_changed(&self) -> Tick {
        *self.ticks.changed
    }
}

impl<T: Component> ChangeDetectionMut for Mut<T> {
    type Inner = T;

    fn bypass_change_detection(&mut self) -> &mut Self::Inner {
        self.column.dense[self.dense_index].downcast_mut().unwrap()
    }

    fn set_changed(&mut self) {
        *self.ticks.changed = self.ticks.this_run;
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

    pub fn insert_components<T: Bundle>(&mut self, entity: Entity, bundle: T, change_tick: Tick) {
        let old_archetype_id = self.entity_archetype.remove(&entity);
        let old_archetype = old_archetype_id.and_then(|id| self.archetypes.get_mut(&id));

        let components = bundle.into_components();
        let mut data = components
            .into_iter()
            .map(|c| (Data::new_dynamic(c), ComponentTicks::new(change_tick)))
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
                        .map(|component| component.0.type_id())
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
                .map(|data| (data.0.type_id(), SharedLock::new(SparseSet::new())))
                .collect();

            self.archetypes.insert(archetype_id, archetype);

            let archetype = self.archetypes.get_mut(&archetype_id).unwrap();

            (archetype, archetype_id)
        };

        for (data, ticks) in data {
            archetype.insert(entity, data, ticks);
        }

        self.entity_archetype.insert(entity, archetype_id);

        if let Some(old_archetype_id) = old_archetype_id {
            if self.archetypes[&old_archetype_id].is_empty() {
                self.archetypes.remove(&old_archetype_id);
            }
        }
    }

    pub fn insert_component<T: Component>(
        &mut self,
        entity: Entity,
        component: T,
        change_tick: Tick,
    ) {
        if self.has_component::<T>(entity) {
            panic!(
                "entity already has component: {}",
                std::any::type_name::<T>()
            );
        }
        self.insert_components(entity, component, change_tick);
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        if !self.has_component::<T>(entity) {
            return None;
        }

        // remove the entity from its current archetype
        let old_archetype_id = self.entity_archetype.remove(&entity)?;
        let old_archetype = self.archetypes.get_mut(&old_archetype_id)?;

        let mut data = old_archetype.remove(entity);

        if old_archetype.is_empty() {
            self.archetypes.remove(&old_archetype_id);
        }

        self.entity_archetype.remove(&entity);

        // remove the component from the data
        let (component, _component_ticks) =
            data.remove(data.iter().position(|data| data.0.is::<T>())?);

        // find the new archetype for the entity
        let new_archetype = self
            .archetypes
            .iter_mut()
            .find(|(_, archetype)| {
                archetype.exclusively_contains_types(
                    &data.iter().map(|data| data.0.type_id()).collect::<Vec<_>>(),
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
                    .map(|data| (data.0.type_id(), SharedLock::new(SparseSet::new())))
                    .collect();

                self.archetypes.insert(archetype_id, archetype);

                let archetype = self.archetypes.get_mut(&archetype_id).unwrap();

                (archetype, archetype_id)
            };

        for (data, ticks) in data {
            new_archetype.insert(entity, data, ticks);
        }

        self.entity_archetype.insert(entity, new_archetype_id);

        let Ok(component) = component.into_data().downcast::<T>() else {
            panic!("downcast failed: expected {}", std::any::type_name::<T>());
        };

        Some(*component)
    }

    pub fn remove_entity(&mut self, entity: Entity) -> Option<Vec<(Data, ComponentTicks)>> {
        let archetype_id = self.entity_archetype.get(&entity)?;

        let archetype = self.archetypes.get_mut(archetype_id)?;

        let data = archetype.remove(entity);

        if archetype.is_empty() {
            self.archetypes.remove(archetype_id);
        }

        self.entity_archetype.remove(&entity);

        Some(data)
    }

    pub fn get_component<T: Component>(
        &self,
        entity: Entity,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Ref<T>> {
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
            let ticks = Ticks {
                added: column.dense_added_ticks[dense_index].read_arc(),
                changed: column.dense_changed_ticks[dense_index].read_arc(),
                last_run,
                this_run,
            };
            Some(Ref::new(dense_index, column, ticks))
        } else {
            None
        }
    }

    pub fn get_component_mut<T: Component>(
        &self,
        entity: Entity,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Mut<T>> {
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
            let ticks = TicksMut {
                added: column.dense_added_ticks[dense_index].write_arc(),
                changed: column.dense_changed_ticks[dense_index].write_arc(),
                last_run,
                this_run,
            };
            Some(Mut::new(dense_index, column, ticks))
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
