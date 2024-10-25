use std::{any::TypeId, hash::BuildHasherDefault, ops::Deref};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use weaver_util::{
    bail,
    lock::{Lock, Read, Write},
    prelude::FxHashMap,
    HashMap, Result, TypeIdMap,
};

use crate::prelude::{
    Bundle, ChangeDetection, ChangeDetectionMut, ComponentTicks, EntityHasher, EntityMap,
    EntitySet, Tick, Ticks, TicksMut,
};

use super::{component::Component, entity::Entity};

pub struct Data {
    pub type_id: TypeId,
    pub data: AtomicRefCell<Box<dyn Component>>,
}

impl Data {
    pub fn new<T: Component>(data: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            data: AtomicRefCell::new(Box::new(data)),
        }
    }

    pub fn new_dynamic(data: Box<dyn Component>) -> Self {
        Self {
            type_id: (*data).as_any().type_id(),
            data: AtomicRefCell::new(data),
        }
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub fn is<T: Component>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    pub fn borrow_data(&self) -> Result<AtomicRef<Box<dyn Component>>> {
        if let Ok(data) = self.data.try_borrow() {
            Ok(data)
        } else {
            bail!("component borrow failed: already borrowed");
        }
    }

    pub fn borrow_data_mut(&self) -> Result<AtomicRefMut<Box<dyn Component>>> {
        if let Ok(data) = self.data.try_borrow_mut() {
            Ok(data)
        } else {
            bail!("component borrow failed: already borrowed");
        }
    }

    pub fn get_data_mut(&mut self) -> &mut dyn Component {
        self.data.get_mut().as_mut()
    }

    pub fn set_data(&mut self, data: Box<dyn Component>) {
        self.data = AtomicRefCell::new(data);
    }

    pub fn into_data(self) -> Box<dyn Component> {
        self.data.into_inner()
    }

    pub fn try_downcast_ref<T: Component>(&self) -> Result<AtomicRef<T>> {
        if !self.is::<T>() {
            bail!("downcast failed: expected {}", std::any::type_name::<T>());
        }

        if let Ok(data) = self.data.try_borrow() {
            Ok(AtomicRef::map(data, |data| {
                data.downcast_ref::<T>().expect("downcast failed")
            }))
        } else {
            bail!("component borrow failed: already borrowed");
        }
    }

    pub fn try_downcast_mut<T: Component>(&self) -> Result<AtomicRefMut<T>> {
        if !self.is::<T>() {
            bail!("downcast failed: expected {}", std::any::type_name::<T>());
        }

        if let Ok(data) = self.data.try_borrow_mut() {
            Ok(AtomicRefMut::map(data, |data| {
                data.downcast_mut::<T>().expect("downcast failed")
            }))
        } else {
            bail!("component borrow failed: already borrowed");
        }
    }
}

#[derive(Default)]
pub struct Column {
    pub(crate) dense: Vec<Data>,
    pub(crate) dense_added_ticks: Vec<Lock<Tick>>,
    pub(crate) dense_changed_ticks: Vec<Lock<Tick>>,
    sparse: HashMap<u32, usize, BuildHasherDefault<EntityHasher>>,
}

impl Column {
    pub fn insert(&mut self, index: usize, data: Data, ticks: ComponentTicks) {
        let dense_index = self.dense.len();

        self.dense.push(data);
        self.dense_added_ticks.push(Lock::new(ticks.added));
        self.dense_changed_ticks.push(Lock::new(ticks.changed));

        self.sparse.insert(index as u32, dense_index);
    }

    pub fn remove(&mut self, index: usize) -> Option<(Data, ComponentTicks)> {
        let dense_index = self.sparse.remove(&(index as u32))?;

        let last = self.dense.len() - 1;
        let data = self.dense.swap_remove(dense_index);

        let added = self.dense_added_ticks.swap_remove(dense_index);
        let changed = self.dense_changed_ticks.swap_remove(dense_index);

        if dense_index != last {
            self.sparse.insert(index as u32, dense_index);
        }

        Some((
            data,
            ComponentTicks {
                added: Lock::into_inner(added),
                changed: Lock::into_inner(changed),
            },
        ))
    }

    pub fn get(&self, index: u32) -> Option<&Data> {
        self.sparse
            .get(&index)
            .map(|&dense_index| &self.dense[dense_index])
    }

    pub fn contains(&self, index: u32) -> bool {
        self.sparse.contains_key(&index)
    }

    pub fn dense_index_of(&self, index: u32) -> Option<usize> {
        self.sparse.get(&index).copied()
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Data> {
        self.dense.iter()
    }

    pub fn sparse_iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.sparse.keys().copied()
    }

    pub fn sparse_iter_with_ticks(
        &self,
    ) -> impl Iterator<Item = (u32, Read<Tick>, Read<Tick>)> + '_ {
        self.sparse.iter().map(move |(index, dense_index)| {
            (
                *index,
                self.dense_added_ticks[*dense_index].read(),
                self.dense_changed_ticks[*dense_index].read(),
            )
        })
    }

    pub fn sparse_iter_with_ticks_mut(
        &self,
    ) -> impl Iterator<Item = (u32, Write<Tick>, Write<Tick>)> + '_ {
        self.sparse.iter().map(move |(index, dense_index)| {
            (
                *index,
                self.dense_added_ticks[*dense_index].write(),
                self.dense_changed_ticks[*dense_index].write(),
            )
        })
    }
}

#[derive(Clone)]
pub struct ColumnRef<'w> {
    column: &'w Column,
}

impl<'w> ColumnRef<'w> {
    pub fn new(column: &'w Column) -> Self {
        Self { column }
    }

    pub fn into_inner(self) -> &'w Column {
        self.column
    }
}

impl<'w> Deref for ColumnRef<'w> {
    type Target = Column;

    fn deref(&self) -> &Self::Target {
        self.column
    }
}

#[derive(Default)]
pub struct Archetype {
    columns: TypeIdMap<Column>,
    entities: EntitySet,
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

        self.columns
            .get_mut(&type_id)
            .unwrap()
            .insert(entity.id() as usize, data, ticks);
    }

    pub fn remove(&mut self, entity: Entity) -> Vec<(Data, ComponentTicks)> {
        self.entities.remove(&entity);

        self.columns
            .values_mut()
            .filter_map(|column| column.remove(entity.id() as usize))
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
            .map(|c| c.contains(entity.id()))
            .unwrap_or(false)
    }

    pub fn contains_component_by_type_id(&self, type_id: TypeId) -> bool {
        self.columns.contains_key(&type_id)
    }

    pub fn contains_entity(&self, entity: Entity) -> bool {
        self.columns
            .values()
            .any(|column| column.contains(entity.id()))
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
    data: AtomicRef<'w, T>,
    ticks: Ticks<'w>,
}

impl<'w, T: Component> Ref<'w, T> {
    pub(crate) fn new(data: AtomicRef<'w, T>, ticks: Ticks<'w>) -> Self {
        Self { data, ticks }
    }

    pub fn into_inner(self) -> AtomicRef<'w, T> {
        self.data
    }
}

impl<'w, T: Component> std::ops::Deref for Ref<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
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
    data: AtomicRefMut<'w, T>,
    ticks: TicksMut<'w>,
}

impl<'w, T: Component> Mut<'w, T> {
    pub(crate) fn new(data: AtomicRefMut<'w, T>, ticks: TicksMut<'w>) -> Self {
        Self { data, ticks }
    }

    pub fn into_inner(self) -> AtomicRefMut<'w, T> {
        self.data
    }
}

impl<'w, T: Component> std::ops::Deref for Mut<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'w, T: Component> std::ops::DerefMut for Mut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        &mut self.data
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
        &mut self.data
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
    archetypes: FxHashMap<ArchetypeId, Archetype>,
    entity_archetype: EntityMap<ArchetypeId>,
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
                .map(|data| (data.0.type_id(), Column::default()))
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
                    .map(|data| (data.0.type_id(), Column::default()))
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
            .contains(entity.id())
        {
            let column = &archetype.columns[&TypeId::of::<T>()];
            let dense_index = column.dense_index_of(entity.id()).unwrap();
            let ticks = Ticks {
                added: column.dense_added_ticks[dense_index].read(),
                changed: column.dense_changed_ticks[dense_index].read(),
                last_run,
                this_run,
            };
            let data = column.dense[dense_index].try_downcast_ref::<T>().unwrap();
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
            .contains(entity.id())
        {
            let column = archetype.columns.get(&TypeId::of::<T>()).unwrap();
            let dense_index = column.dense_index_of(entity.id()).unwrap();
            let ticks = TicksMut {
                added: column.dense_added_ticks[dense_index].write(),
                changed: column.dense_changed_ticks[dense_index].write(),
                last_run,
                this_run,
            };
            let data = column.dense[dense_index].try_downcast_mut::<T>().unwrap();
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
