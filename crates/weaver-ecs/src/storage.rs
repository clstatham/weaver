use std::{
    any::TypeId,
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicIsize, Ordering},
};

use weaver_util::prelude::*;

use crate::{
    bundle::{Bundle, ComponentBundle},
    change_detection::{ChangeDetection, ChangeDetectionMut, ComponentTicks, Tick},
    component::{Component, ComponentVec},
    entity::{Entity, EntityMap},
};

/// One component type's data and change ticks within an archetype, indexed by row.
///
/// The cells let a [`Query`](crate::query::Query) read or write rows through a shared
/// `&Components`. That is only sound while the query holds the matching borrow flag
/// (see [`Components::try_borrow`]); structural changes go through `&mut Components` instead.
pub(crate) struct Column {
    data: UnsafeCell<ComponentVec>,
    ticks: UnsafeCell<Vec<ComponentTicks>>,
}

// SAFETY: shared access to the cells is gated by the per-type borrow flags in `Components`:
// any number of readers, or a single writer, never both.
unsafe impl Sync for Column {}

impl Column {
    fn new(data: ComponentVec) -> Self {
        Self {
            data: UnsafeCell::new(data),
            ticks: UnsafeCell::new(Vec::new()),
        }
    }

    /// # Safety
    ///
    /// The caller must hold a read borrow of this column's component type, and `T` must be that type.
    pub(crate) unsafe fn data_slice<T: Component>(&self) -> &[T] {
        // SAFETY: no writer exists while a read borrow is held, and the caller vouches for `T`
        unsafe { (*self.data.get()).as_slice_unchecked::<T>() }
    }

    /// # Safety
    ///
    /// The caller must hold a read borrow of this column's component type.
    pub(crate) unsafe fn ticks_slice(&self) -> &[ComponentTicks] {
        // SAFETY: no writer exists while a read borrow is held
        unsafe { &*self.ticks.get() }
    }

    /// # Safety
    ///
    /// The caller must hold the write borrow of this column's component type, `T` must be that
    /// type, and no other reference into this column may be alive.
    pub(crate) unsafe fn data_ptr<T: Component>(&self) -> *mut T {
        // SAFETY: structural changes need `&mut Components`, so nothing mutates the vec itself
        // while we read its buffer pointer; the write borrow covers the elements
        unsafe { (*self.data.get()).as_ptr().cast::<T>() }
    }

    /// # Safety
    ///
    /// Same as [`Column::data_ptr`].
    pub(crate) unsafe fn ticks_ptr(&self) -> *mut ComponentTicks {
        // SAFETY: the write borrow makes this the only access to the column
        unsafe { (*self.ticks.get()).as_mut_ptr() }
    }
}

#[derive(Default)]
pub struct Archetype {
    /// Sorted, so lookups can binary search.
    data_types: Vec<TypeId>,
    columns: Vec<Column>,
    entities: Vec<Entity>,
}

impl Archetype {
    pub fn new_for_bundle<T: Bundle>() -> Self {
        let mut vecs = T::empty_vecs();
        vecs.sort_unstable_by_key(|vec| vec.element_typeid());
        let mut data_types = T::component_type_ids();
        data_types.sort_unstable();

        Self {
            data_types,
            columns: vecs.into_iter().map(Column::new).collect(),
            entities: Vec::new(),
        }
    }

    pub fn data_types(&self) -> &[TypeId] {
        &self.data_types
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn index_of(&self, ty: TypeId) -> Option<usize> {
        self.data_types.binary_search(&ty).ok()
    }

    /// Finds an entity's row with a linear scan. Prefer [`Components::location`], which is O(1).
    pub fn entity_index(&self, entity: Entity) -> Option<usize> {
        self.entities.iter().position(|&id| id == entity)
    }

    pub fn has<T: Component>(&self) -> bool {
        self.index_of(TypeId::of::<T>()).is_some()
    }

    pub(crate) fn column_of<T: Component>(&self) -> Option<&Column> {
        self.index_of(TypeId::of::<T>())
            .map(|index| &self.columns[index])
    }

    pub fn exactly_matches_bundle<T: Bundle>(&self) -> bool {
        let mut bundle = T::component_type_ids();
        bundle.sort_unstable();
        self.data_types == bundle
    }

    pub fn partially_matches_bundle<T: Bundle>(&self) -> bool {
        let bundle = T::component_type_ids();
        self.data_types.iter().all(|id| bundle.contains(id))
    }

    pub fn exactly_matches_type_ids(&self, data_types: impl IntoIterator<Item = TypeId>) -> bool {
        let mut data_types = data_types.into_iter().collect::<Vec<_>>();
        data_types.sort_unstable();
        self.data_types == data_types
    }

    pub fn partially_matches_type_ids(&self, data_types: impl IntoIterator<Item = TypeId>) -> bool {
        data_types.into_iter().all(|id| self.index_of(id).is_some())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchetypeId(u64);

impl ArchetypeId {
    pub fn from_u64(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

/// Where an entity's components live: its archetype and its row within that archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityLocation {
    pub archetype: ArchetypeId,
    pub row: usize,
}

/// The kind of access a [`Query`](crate::query::Query) term needs to a component type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    Read,
    Write,
}

#[derive(Default)]
pub struct Components {
    // Note: This Vec never shrinks. This is intentional to avoid changing the ArchetypeId of existing archetypes. Empty archetypes are kept initialized in memory for potential reuse later.
    archetypes: Vec<Archetype>,
    /// Sorted component types -> archetype, mirroring each archetype's `data_types`.
    archetype_by_types: FxHashMap<Box<[TypeId]>, ArchetypeId>,
    entity_location: EntityMap<EntityLocation>,
    /// One borrow flag per stored component type: `n > 0` is `n` readers, `-1` is one writer.
    /// Entries are only added through `&mut self`, so the map itself is never mutated while a query holds borrows.
    borrows: TypeIdMap<AtomicIsize>,
}

impl Components {
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes an entity from the storage and returns the components that were removed.
    ///
    /// Panics if the entity does not exist.
    pub(crate) fn remove_entity(&mut self, entity: Entity) -> ComponentBundle {
        let location = self
            .entity_location
            .remove(&entity)
            .expect("Entity does not exist");
        let archetype = &mut self.archetypes[location.archetype.as_usize()];
        let row = location.row;

        let mut components = Vec::with_capacity(archetype.columns.len());
        let mut ticks = Vec::with_capacity(archetype.columns.len());
        for column in archetype.columns.iter_mut() {
            let data = column.data.get_mut();
            let mut tmp = data.clone_empty();
            data.swap_remove_into(row, &mut tmp);
            components.push(tmp);
            ticks.push(column.ticks.get_mut().swap_remove(row));
        }

        archetype.entities.swap_remove(row);
        // the last entity was moved into the hole, so its row changed
        if let Some(&moved) = archetype.entities.get(row) {
            self.entity_location.get_mut(&moved).unwrap().row = row;
        }

        ComponentBundle {
            types: archetype.data_types.clone(),
            components,
            ticks,
        }
    }

    pub(crate) fn insert_entity(&mut self, entity: Entity, components: ComponentBundle) {
        assert!(
            !self.entity_location.contains_key(&entity),
            "Entity already exists"
        );
        debug_assert!(components.types.is_sorted());

        let archetype_id = match self.archetype_by_types.get(components.types.as_slice()) {
            Some(&id) => id, // archetype already exists
            None => {
                // create a new archetype for this bundle
                let archetype = Archetype {
                    data_types: components.types.clone(),
                    columns: components
                        .empty_vecs()
                        .into_iter()
                        .map(Column::new)
                        .collect(),
                    entities: Vec::new(),
                };
                for &ty in &components.types {
                    self.borrows
                        .entry(ty)
                        .or_insert_with(|| AtomicIsize::new(0));
                }
                let id = ArchetypeId::from_u64(self.archetypes.len() as u64);
                self.archetypes.push(archetype);
                self.archetype_by_types
                    .insert(components.types.clone().into_boxed_slice(), id);
                id
            }
        };

        let archetype = &mut self.archetypes[archetype_id.as_usize()];
        let row = archetype.entities.len();
        archetype.entities.push(entity);

        for ((column, mut component), tick) in archetype
            .columns
            .iter_mut()
            .zip(components.components)
            .zip(components.ticks)
        {
            assert!(component.pop_into(column.data.get_mut()));
            column.ticks.get_mut().push(tick);
        }

        self.entity_location.insert(
            entity,
            EntityLocation {
                archetype: archetype_id,
                row,
            },
        );
    }

    pub fn insert_bundle<T: Bundle>(&mut self, entity: Entity, bundle: T, tick: Tick) {
        let mut components = ComponentBundle::from_tuple(bundle, tick);
        if self.entity_location.contains_key(&entity) {
            let old_comps = self.remove_entity(entity);
            components.union(old_comps);
        }
        self.insert_entity(entity, components);
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        let mut components = self.remove_entity(entity);
        let removed = components.remove::<T>();
        self.insert_entity(entity, components);
        removed
    }

    pub fn location(&self, entity: Entity) -> Option<EntityLocation> {
        self.entity_location.get(&entity).copied()
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        let location = self.location(entity).expect("Entity does not exist");
        self.archetypes[location.archetype.as_usize()].has::<T>()
    }

    fn component_ticks<T: Component>(&self, entity: Entity) -> ComponentTicks {
        let location = self.location(entity).expect("Entity does not exist");
        let column = self.archetypes[location.archetype.as_usize()]
            .column_of::<T>()
            .expect("Entity does not have the component");
        let ty = TypeId::of::<T>();
        assert!(
            self.try_borrow(ty, Access::Read),
            "{} is mutably borrowed by a live Query",
            std::any::type_name::<T>()
        );
        // SAFETY: we hold a read borrow of T
        let ticks = unsafe { column.ticks_slice()[location.row] };
        self.release(ty, Access::Read);
        ticks
    }

    pub fn component_added<T: Component>(
        &self,
        entity: Entity,
        last_run: Tick,
        this_run: Tick,
    ) -> bool {
        self.component_ticks::<T>(entity)
            .is_added(last_run, this_run)
    }

    pub fn component_changed<T: Component>(
        &self,
        entity: Entity,
        last_run: Tick,
        this_run: Tick,
    ) -> bool {
        self.component_ticks::<T>(entity)
            .is_changed(last_run, this_run)
    }

    pub fn archetype_iter(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.iter()
    }

    pub fn archetypes(&self) -> &[Archetype] {
        &self.archetypes
    }

    /// Tries to take a read or write borrow of every column of component type `ty`.
    /// A type that was never stored has no columns, so borrowing it always succeeds.
    pub(crate) fn try_borrow(&self, ty: TypeId, access: Access) -> bool {
        let Some(flag) = self.borrows.get(&ty) else {
            return true;
        };
        match access {
            Access::Read => flag
                .fetch_update(Ordering::Acquire, Ordering::Relaxed, |n| {
                    (n >= 0).then_some(n + 1)
                })
                .is_ok(),
            Access::Write => flag
                .compare_exchange(0, -1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok(),
        }
    }

    /// Releases a borrow taken with [`Components::try_borrow`].
    pub(crate) fn release(&self, ty: TypeId, access: Access) {
        let Some(flag) = self.borrows.get(&ty) else {
            return;
        };
        match access {
            Access::Read => {
                flag.fetch_sub(1, Ordering::Release);
            }
            Access::Write => flag.store(0, Ordering::Release),
        }
    }
}

pub struct Ref<'a, T: Component> {
    item: &'a T,
    last_run: Tick,
    this_run: Tick,
    ticks: &'a ComponentTicks,
}

impl<'a, T: Component> Ref<'a, T> {
    pub(crate) fn new(
        item: &'a T,
        last_run: Tick,
        this_run: Tick,
        ticks: &'a ComponentTicks,
    ) -> Self {
        Self {
            item,
            last_run,
            this_run,
            ticks,
        }
    }

    pub fn get(&self) -> &T {
        self.item
    }

    pub fn get_ticks(&self) -> &ComponentTicks {
        self.ticks
    }
}

impl<T: Component> ChangeDetection for Ref<'_, T> {
    fn is_added(&self) -> bool {
        self.get_ticks().is_added(self.last_run, self.this_run)
    }

    fn is_changed(&self) -> bool {
        self.get_ticks().is_changed(self.last_run, self.this_run)
    }

    fn last_changed(&self) -> Tick {
        self.get_ticks().changed
    }
}

impl<T: Component> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

pub struct Mut<'a, T: Component> {
    item: &'a mut T,
    last_run: Tick,
    this_run: Tick,
    ticks: &'a mut ComponentTicks,
}

impl<'a, T: Component> Mut<'a, T> {
    pub(crate) fn new(
        column: &'a mut T,
        last_run: Tick,
        this_run: Tick,
        ticks: &'a mut ComponentTicks,
    ) -> Self {
        Self {
            item: column,
            last_run,
            this_run,
            ticks,
        }
    }

    pub fn get(&self) -> &T {
        self.item
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.item
    }

    pub fn get_ticks(&self) -> &ComponentTicks {
        self.ticks
    }

    pub fn get_ticks_mut(&mut self) -> &mut ComponentTicks {
        self.ticks
    }
}

impl<T: Component> ChangeDetection for Mut<'_, T> {
    fn is_added(&self) -> bool {
        self.get_ticks().is_added(self.last_run, self.this_run)
    }

    fn is_changed(&self) -> bool {
        self.get_ticks().is_changed(self.last_run, self.this_run)
    }

    fn last_changed(&self) -> Tick {
        self.get_ticks().changed
    }
}

impl<T: Component> ChangeDetectionMut for Mut<'_, T> {
    type Inner = T;

    fn set_changed(&mut self) {
        let tick = self.this_run;
        self.get_ticks_mut().set_changed(tick);
    }

    fn set_last_changed(&mut self, tick: Tick) {
        self.get_ticks_mut().changed = tick;
    }

    fn bypass_change_detection(&mut self) -> &mut Self::Inner {
        self.get_mut()
    }
}

impl<T: Component> Deref for Mut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T: Component> DerefMut for Mut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        self.get_mut()
    }
}
