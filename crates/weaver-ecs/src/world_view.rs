use std::{any::TypeId, marker::PhantomData};

use weaver_util::FxHashSet;

use crate::{
    entity::Entity,
    query::{QueryFetch, QueryFetchAccess, QueryFilter},
    system::{SystemAccess, SystemParam},
    world::UnsafeWorldCell,
};

/// A view of a [`World`] that eagerly searches the world for entities matching a set of component filters.
///
/// This differs from a [`Query`] in that a [`WorldView`] gathers all entities matching the component filters at the time of creation,
/// and does not update as the world changes, whereas a [`Query`] lazily searches the world for entities matching the component filters
/// each time it is iterated.
///
/// [`World`]: crate::world::World
/// [`Query`]: crate::query::Query
pub struct WorldView<'w, Q: QueryFetch, F: QueryFilter = ()> {
    world: UnsafeWorldCell<'w>,
    entities: Vec<Entity>,
    read: Vec<TypeId>,
    write: Vec<TypeId>,
    with: Vec<TypeId>,
    without: Vec<TypeId>,

    _query_fetch: PhantomData<Q>,
    _query_filter: PhantomData<F>,
}

impl<'w, Q, F> WorldView<'w, Q, F>
where
    Q: QueryFetch,
    F: QueryFilter,
{
    /// Creates a new [`WorldView`] from a [`QueryFetch`] and a [`QueryFilter`].
    ///
    /// This will eagerly search the world for entities matching the component filters,
    /// which can be useful for cases where you want to gather all entities matching the filters at once,
    /// and do not need the view to update as the world changes.
    pub fn new(world: UnsafeWorldCell<'w>) -> Self {
        let mut entities = Vec::new();
        let storage = unsafe { world.world().storage() };

        let fetch_access = Q::access();
        let filter_access = F::access();

        let mut read = Vec::new();
        let mut write = Vec::new();
        let mut with = Vec::new();
        let mut without = Vec::new();

        for (type_id, access) in fetch_access.iter() {
            match access {
                crate::query::QueryFetchAccess::ReadOnly => read.push(*type_id),
                crate::query::QueryFetchAccess::ReadWrite => write.push(*type_id),
            }
        }

        for (type_id, access) in filter_access.iter() {
            match access {
                crate::query::QueryFilterAccess::With => with.push(*type_id),
                crate::query::QueryFilterAccess::Without => without.push(*type_id),
            }
        }

        'entity_iter: for entity in storage.entity_iter() {
            for &type_id in &read {
                if !storage.has_component_by_type_id(entity, type_id) {
                    continue 'entity_iter;
                }
            }

            for &type_id in &write {
                if !storage.has_component_by_type_id(entity, type_id) {
                    continue 'entity_iter;
                }
            }

            for &type_id in &with {
                if !storage.has_component_by_type_id(entity, type_id) {
                    continue 'entity_iter;
                }
            }

            for &type_id in &without {
                if storage.has_component_by_type_id(entity, type_id) {
                    continue 'entity_iter;
                }
            }

            entities.push(entity);
        }

        Self {
            world,
            entities,
            read,
            write,
            with,
            without,

            _query_fetch: PhantomData,
            _query_filter: PhantomData,
        }
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn reads(&self) -> &[TypeId] {
        &self.read
    }

    pub fn writes(&self) -> &[TypeId] {
        &self.write
    }

    pub fn withs(&self) -> &[TypeId] {
        &self.with
    }

    pub fn withouts(&self) -> &[TypeId] {
        &self.without
    }

    pub fn iter(&self) -> WorldViewIter<'w, '_, Q, F> {
        WorldViewIter {
            world_view: self,
            index: 0,
        }
    }

    pub fn get(&self, entity: Entity) -> Option<Q::Item<'w>> {
        if self.entities.contains(&entity) {
            unsafe { Q::fetch::<F>(self.world.world(), entity) }
        } else {
            None
        }
    }
}

/// An iterator over the entities in a [`WorldView`].
///
/// This iterator will yield an [`EntityView`] for each entity in the [`WorldView`].
pub struct WorldViewIter<'w, 'v, Q: QueryFetch, F: QueryFilter> {
    world_view: &'v WorldView<'w, Q, F>,
    index: usize,
}

impl<'w, Q, F> Iterator for WorldViewIter<'w, '_, Q, F>
where
    Q: QueryFetch,
    F: QueryFilter,
{
    type Item = (Entity, Q::Item<'w>);

    fn next(&mut self) -> Option<(Entity, Q::Item<'w>)> {
        if self.index < self.world_view.entities.len() {
            let entity = self.world_view.entities[self.index];
            self.index += 1;
            self.world_view.get(entity).map(|item| (entity, item))
        } else {
            None
        }
    }
}

unsafe impl<Q, F> SystemParam for WorldView<'_, Q, F>
where
    Q: QueryFetch,
    F: QueryFilter,
{
    type State = ();

    type Item<'w, 's> = WorldView<'w, Q, F>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::default(),
            resources_written: FxHashSet::default(),
            components_read: Q::access()
                .iter()
                .filter_map(|(ty, access)| {
                    if let QueryFetchAccess::ReadOnly = access {
                        Some(*ty)
                    } else {
                        None
                    }
                })
                .collect(),
            components_written: Q::access()
                .iter()
                .filter_map(|(ty, access)| {
                    if let QueryFetchAccess::ReadWrite = access {
                        Some(*ty)
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }

    fn init_state(_world: &mut crate::prelude::World) -> Self::State {}

    unsafe fn fetch<'w, 's>(
        _state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        WorldView::new(world)
    }
}
