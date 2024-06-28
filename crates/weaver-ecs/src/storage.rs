use std::{
    any::TypeId,
    cell::UnsafeCell,
    collections::{HashMap, HashSet},
    ops::Deref,
};

use weaver_util::lock::{Lock, Read, Write};

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
    pub(crate) dense_added_ticks: Vec<Lock<Tick>>,
    pub(crate) dense_changed_ticks: Vec<Lock<Tick>>,
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
                self.dense_added_ticks.push(Lock::new(ticks.added));
                self.dense_changed_ticks.push(Lock::new(ticks.changed));

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

    pub fn iter_with_ticks(&self) -> impl Iterator<Item = (&T, Read<Tick>, Read<Tick>)> + '_ {
        self.dense
            .iter()
            .zip(self.dense_added_ticks.iter())
            .zip(self.dense_changed_ticks.iter())
            .map(|((data, added), changed)| (data, added.read(), changed.read()))
    }

    pub fn sparse_iter(&self) -> std::slice::Iter<'_, usize> {
        self.indices.iter()
    }

    pub fn sparse_iter_with_ticks(
        &self,
    ) -> impl Iterator<Item = (usize, Read<Tick>, Read<Tick>)> + '_ {
        self.indices.iter().map(move |&index| {
            let dense_index = self.dense_index_of(index).unwrap();
            (
                index,
                self.dense_added_ticks[dense_index].read(),
                self.dense_changed_ticks[dense_index].read(),
            )
        })
    }

    pub fn sparse_iter_with_ticks_mut(
        &self,
    ) -> impl Iterator<Item = (usize, Write<Tick>, Write<Tick>)> + '_ {
        self.indices.iter().map(|&index| {
            let dense_index = self.dense_index_of(index).unwrap();
            (
                index,
                self.dense_added_ticks[dense_index].write(),
                self.dense_changed_ticks[dense_index].write(),
            )
        })
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
pub struct ColumnRef<'w> {
    column: &'w SparseSet<UnsafeCell<Data>>,
}

unsafe impl Send for ColumnRef<'_> {}
unsafe impl Sync for ColumnRef<'_> {}

impl<'w> ColumnRef<'w> {
    pub fn new(column: &'w SparseSet<UnsafeCell<Data>>) -> Self {
        Self { column }
    }

    pub fn into_inner(self) -> &'w SparseSet<UnsafeCell<Data>> {
        self.column
    }
}

impl<'w> Deref for ColumnRef<'w> {
    type Target = SparseSet<UnsafeCell<Data>>;

    fn deref(&self) -> &Self::Target {
        self.column
    }
}

#[derive(Default)]
pub struct Archetype {
    columns: HashMap<TypeId, SparseSet<UnsafeCell<Data>>>,
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

        self.columns.get_mut(&type_id).unwrap().insert(
            entity.as_usize(),
            UnsafeCell::new(data),
            ticks,
        );
    }

    pub fn remove(&mut self, entity: Entity) -> Vec<(Data, ComponentTicks)> {
        self.entities.remove(&entity);

        self.columns
            .values_mut()
            .filter_map(|column| {
                column
                    .remove(entity.as_usize())
                    .map(|(data, ticks)| (data.into_inner(), ticks))
            })
            .collect()
    }

    #[inline]
    pub fn get_column<T: Component>(&self) -> Option<ColumnRef<'_>> {
        self.get_column_by_type_id(TypeId::of::<T>())
    }

    pub fn get_column_by_type_id(&self, type_id: TypeId) -> Option<ColumnRef<'_>> {
        Some(ColumnRef::new(self.columns.get(&type_id)?))
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        self.has_component_by_type_id(entity, TypeId::of::<T>())
    }

    pub fn has_component_by_type_id(&self, entity: Entity, type_id: TypeId) -> bool {
        self.columns
            .get(&type_id)
            .map(|c| c.contains(entity.as_usize()))
            .unwrap_or(false)
    }

    pub fn contains_component_by_type_id(&self, type_id: TypeId) -> bool {
        self.columns.contains_key(&type_id)
    }

    pub fn contains_entity(&self, entity: Entity) -> bool {
        self.columns
            .values()
            .any(|column| column.contains(entity.as_usize()))
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
            .map(|(type_id, column)| (type_id, ColumnRef::new(column)))
    }

    pub fn column_iter_mut(&mut self) -> impl Iterator<Item = (&TypeId, ColumnRef)> + '_ {
        self.columns
            .iter_mut()
            .map(|(type_id, column)| (type_id, ColumnRef::new(column)))
    }
}

pub struct Ref<'w, T: Component> {
    data: &'w T,
    ticks: Ticks<'w>,
}

impl<'w, T: Component> Ref<'w, T> {
    pub(crate) fn new(data: &'w T, ticks: Ticks<'w>) -> Self {
        Self { data, ticks }
    }

    pub fn into_inner(self) -> &'w T {
        self.data
    }
}

impl<'w, T: Component> std::ops::Deref for Ref<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'w, T: Component> ChangeDetection for Ref<'w, T> {
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

pub struct Mut<'w, T: Component> {
    data: &'w mut T,
    ticks: TicksMut<'w>,
}

impl<'w, T: Component> Mut<'w, T> {
    pub(crate) fn new(data: &'w mut T, ticks: TicksMut<'w>) -> Self {
        Self { data, ticks }
    }

    pub fn into_inner(self) -> &'w mut T {
        self.data
    }
}

impl<'w, T: Component> std::ops::Deref for Mut<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'w, T: Component> std::ops::DerefMut for Mut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        self.data
    }
}

impl<'w, T: Component> ChangeDetection for Mut<'w, T> {
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

impl<'w, T: Component> ChangeDetectionMut for Mut<'w, T> {
    type Inner = T;

    fn bypass_change_detection(&mut self) -> &mut Self::Inner {
        self.data
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

    pub fn insert_bundle<T: Bundle>(&mut self, entity: Entity, bundle: T, change_tick: Tick) {
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
                .map(|data| (data.0.type_id(), SparseSet::new()))
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
        last_run: Tick,
        this_run: Tick,
    ) {
        if let Some(mut existing) = self.get_component_mut::<T>(entity, last_run, this_run) {
            // panic!(
            //     "entity already has component: {}",
            //     std::any::type_name::<T>()
            // );
            *existing = component;
            return;
        }
        self.insert_bundle(entity, component, this_run);
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
                    .map(|data| (data.0.type_id(), SparseSet::new()))
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
            .contains(entity.as_usize())
        {
            let column = &archetype.columns[&TypeId::of::<T>()];
            let dense_index = column.dense_index_of(entity.as_usize()).unwrap();
            let ticks = Ticks {
                added: column.dense_added_ticks[dense_index].read(),
                changed: column.dense_changed_ticks[dense_index].read(),
                last_run,
                this_run,
            };
            let data = unsafe {
                (*column.dense[dense_index].get())
                    .downcast_ref::<T>()
                    .unwrap()
            };
            Some(Ref::new(data, ticks))
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
            .contains(entity.as_usize())
        {
            let column = archetype.columns.get(&TypeId::of::<T>()).unwrap();
            let dense_index = column.dense_index_of(entity.as_usize()).unwrap();
            let ticks = TicksMut {
                added: column.dense_added_ticks[dense_index].write(),
                changed: column.dense_changed_ticks[dense_index].write(),
                last_run,
                this_run,
            };
            let data = unsafe {
                (*column.dense[dense_index].get())
                    .downcast_mut::<T>()
                    .unwrap()
            };
            Some(Mut::new(data, ticks))
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

    pub fn get_archetype_mut(&mut self, entity: Entity) -> Option<&mut Archetype> {
        self.entity_archetype
            .get(&entity)
            .and_then(|archetype_id| self.archetypes.get_mut(archetype_id))
    }
}
