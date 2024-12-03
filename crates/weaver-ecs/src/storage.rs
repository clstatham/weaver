use std::{
    any::TypeId,
    ops::{Deref, DerefMut},
};

use weaver_util::{lock::SharedLock, prelude::log};

use crate::{
    bundle::{Bundle, ComponentBundle},
    change_detection::{ChangeDetection, ChangeDetectionMut, ComponentTicks, Tick},
    component::{Component, ComponentVec},
    entity::{Entity, EntityMap},
    loan::LoanStorage,
};

#[derive(Default)]
pub struct Archetype {
    data_types: Vec<TypeId>,
    columns: Vec<SharedLock<LoanStorage<ComponentVec>>>,
    ticks: Vec<SharedLock<LoanStorage<Vec<ComponentTicks>>>>,
    entity_id_lookup: Vec<Entity>,
}

impl Archetype {
    pub fn new_for_bundle<T: Bundle>() -> Self {
        let mut vecs = T::empty_vecs();
        vecs.sort_unstable_by_key(|vec| vec.element_typeid());
        let mut data_types = T::component_type_ids();
        data_types.sort_unstable();
        let columns = vecs
            .into_iter()
            .map(LoanStorage::new)
            .map(SharedLock::new)
            .collect::<Vec<_>>();

        let mut ticks = Vec::new();
        for _ in &columns {
            ticks.push(SharedLock::new(LoanStorage::new(vec![])));
        }

        Self {
            data_types,
            columns,
            ticks,
            entity_id_lookup: Vec::new(),
        }
    }

    pub fn columns(&self) -> &[SharedLock<LoanStorage<ComponentVec>>] {
        &self.columns
    }

    pub fn ticks(&self) -> &[SharedLock<LoanStorage<Vec<ComponentTicks>>>] {
        &self.ticks
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entity_id_lookup.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.entity_id_lookup.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entity_id_lookup.is_empty()
    }

    pub fn index_of(&self, ty: TypeId) -> Option<usize> {
        self.data_types.iter().position(|&id| id == ty)
    }

    pub fn entity_index(&self, entity: Entity) -> Option<usize> {
        self.entity_id_lookup.iter().position(|&id| id == entity)
    }

    pub fn has<T: Component>(&self) -> bool {
        self.index_of(TypeId::of::<T>()).is_some()
    }

    pub fn exactly_matches_bundle<T: Bundle>(&self) -> bool {
        let mut sorted = self.data_types.clone();
        sorted.sort_unstable();
        let mut bundle = T::component_type_ids();
        bundle.sort_unstable();
        sorted == bundle
    }

    pub fn partially_matches_bundle<T: Bundle>(&self) -> bool {
        let bundle = T::component_type_ids();
        self.data_types.iter().all(|id| bundle.contains(id))
    }

    pub fn exactly_matches_type_ids(&self, data_types: impl IntoIterator<Item = TypeId>) -> bool {
        let mut sorted = self.data_types.clone();
        sorted.sort_unstable();
        let mut data_types = data_types.into_iter().collect::<Vec<_>>();
        data_types.sort_unstable();
        sorted == data_types
    }

    pub fn partially_matches_type_ids(&self, data_types: impl IntoIterator<Item = TypeId>) -> bool {
        data_types
            .into_iter()
            .all(|id| self.data_types.contains(&id))
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

#[derive(Default)]
pub struct Components {
    // Note: This Vec never shrinks. This is intentional to avoid changing the ArchetypeId of existing archetypes. Empty archetypes are kept initialized in memory for potential reuse later.
    archetypes: Vec<Archetype>,
    entity_archetype: EntityMap<ArchetypeId>,
}

impl Components {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get_archetype_id_for_type_ids(
        &self,
        data_types: impl IntoIterator<Item = TypeId>,
    ) -> Option<ArchetypeId> {
        let data_types = data_types.into_iter().collect::<Vec<_>>();
        self.archetypes
            .iter()
            .enumerate()
            .find_map(|(i, archetype)| {
                if archetype.exactly_matches_type_ids(data_types.iter().copied()) {
                    Some(ArchetypeId::from_u64(i as u64))
                } else {
                    None
                }
            })
    }

    /// Removes an entity from the storage and returns the components that were removed.
    ///
    /// If the entity cannot be removed, returns `None`.
    pub(crate) fn remove_entity(&mut self, entity: Entity) -> ComponentBundle {
        assert!(
            self.entity_archetype.contains_key(&entity),
            "Entity does not exist"
        );
        let archetype_id = self.entity_archetype.remove(&entity).unwrap();
        let archetype = &mut self.archetypes[archetype_id.as_usize()];
        let entity_index = archetype.entity_index(entity).unwrap();

        // make sure we can mutably loan all columns before making any changes
        for column in archetype.columns() {
            let mut column = column.write();
            if column.loan_mut().is_none() {
                log::error!("Cannot remove entity because a component is already borrowed");
                panic!("Cannot remove entity because a component is already borrowed");
            }
        }

        let mut components = Vec::new();

        for column in archetype.columns() {
            let mut column = column.write();
            let mut tmp = column.loan().unwrap().clone_empty();
            let mut column = column.loan_mut().unwrap();
            let component = column.swap_remove(entity_index);
            tmp.push(component);
            components.push(tmp);
        }

        let mut ticks = Vec::new();
        for tick_column in archetype.ticks.iter() {
            let mut tick_column = tick_column.write();
            let mut tick_column = tick_column.loan_mut().unwrap();
            let tick = tick_column.swap_remove(entity_index);
            ticks.push(tick);
        }

        archetype.entity_id_lookup.swap_remove(entity_index);
        ComponentBundle {
            types: archetype.data_types.clone(),
            components,
            ticks,
        }
    }

    pub(crate) fn insert_entity(&mut self, entity: Entity, components: ComponentBundle) {
        assert!(
            !self.entity_archetype.contains_key(&entity),
            "Entity already exists"
        );

        let maybe_archetype_id =
            self.get_archetype_id_for_type_ids(components.types.iter().copied());
        let archetype_id = match maybe_archetype_id {
            Some(id) => id, // archetype already exists
            None => {
                // create a new archetype for this bundle
                let archetype = Archetype {
                    data_types: components.types.clone(),
                    columns: components
                        .empty_vecs()
                        .into_iter()
                        .map(LoanStorage::new)
                        .map(SharedLock::new)
                        .collect(),
                    ticks: components
                        .ticks
                        .iter()
                        .map(|_| SharedLock::new(LoanStorage::new(vec![])))
                        .collect(),
                    entity_id_lookup: Vec::new(),
                };
                let id = ArchetypeId::from_u64(self.archetypes.len() as u64);
                self.archetypes.push(archetype);
                id
            }
        };

        let archetype = &mut self.archetypes[archetype_id.as_usize()];
        archetype.entity_id_lookup.push(entity);

        for (column, mut component) in archetype.columns.iter_mut().zip(components.components) {
            let mut column = column.write();
            column.loan_mut().unwrap().push(component.pop().unwrap());
        }

        for (tick_column, tick) in archetype.ticks.iter_mut().zip(components.ticks) {
            let mut tick_column = tick_column.write();
            tick_column.loan_mut().unwrap().push(tick);
        }

        self.entity_archetype.insert(entity, archetype_id);
    }

    pub fn insert_bundle<T: Bundle>(&mut self, entity: Entity, bundle: T, tick: Tick) {
        let mut components = ComponentBundle::from_tuple(bundle, tick);
        if self.entity_archetype.contains_key(&entity) {
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

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        let archetype_id = self.entity_archetype.get(&entity).unwrap();
        let archetype = &self.archetypes[archetype_id.as_usize()];
        archetype.entity_index(entity).is_some() && archetype.has::<T>()
    }

    pub fn component_added<T: Component>(
        &self,
        entity: Entity,
        last_run: Tick,
        this_run: Tick,
    ) -> bool {
        let archetype_id = self.entity_archetype.get(&entity).unwrap();
        let archetype = &self.archetypes[archetype_id.as_usize()];
        let entity_index = archetype.entity_index(entity).unwrap();
        let column_index = archetype.index_of(TypeId::of::<T>()).unwrap();
        let mut tick_column = archetype.ticks[column_index].write();
        let mut tick_column = tick_column.loan_mut().unwrap();
        let ticks = tick_column.get_mut(entity_index).unwrap();
        ticks.is_added(last_run, this_run)
    }

    pub fn component_changed<T: Component>(
        &self,
        entity: Entity,
        last_run: Tick,
        this_run: Tick,
    ) -> bool {
        let archetype_id = self.entity_archetype.get(&entity).unwrap();
        let archetype = &self.archetypes[archetype_id.as_usize()];
        let entity_index = archetype.entity_index(entity).unwrap();
        let column_index = archetype.index_of(TypeId::of::<T>()).unwrap();
        let mut tick_column = archetype.ticks[column_index].write();
        let mut tick_column = tick_column.loan_mut().unwrap();
        let ticks = tick_column.get_mut(entity_index).unwrap();
        ticks.is_changed(last_run, this_run)
    }

    pub fn archetype_iter(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.iter()
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
