use std::any::TypeId;

use weaver_util::prelude::*;

use crate::{
    change_detection::{ComponentTicks, Tick},
    entity::Entity,
    prelude::{Archetype, Component, ComponentVec, SystemAccess, SystemParam},
    storage::{Components, Mut, Ref},
};

use super::world::World;

pub struct ReadLockedColumns {
    pub column: OwnedRead<ComponentVec>,
    pub ticks: OwnedRead<Vec<ComponentTicks>>,
    pub entities: Vec<Entity>,
    pub entity_indices: Vec<usize>,
}

pub struct WriteLockedColumns {
    pub column: OwnedWrite<ComponentVec>,
    pub ticks: OwnedWrite<Vec<ComponentTicks>>,
    pub entities: Vec<Entity>,
    pub entity_indices: Vec<usize>,
}

pub trait Queryable: Send + Sync {
    type LockedColumns: Send + Sync;
    type Item<'a>: Send + Sync;

    fn reads() -> Vec<TypeId>;

    fn writes() -> Vec<TypeId>;

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns;

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>>;

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>>;
}

pub type QueryableItem<'a, T> = <T as Queryable>::Item<'a>;

impl Queryable for Entity {
    type LockedColumns = Vec<Entity>;
    type Item<'a> = Entity;

    fn reads() -> Vec<TypeId> {
        vec![]
    }

    fn writes() -> Vec<TypeId> {
        vec![]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        archetype.entity_iter().collect()
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        _last_run: Tick,
        _this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.iter().copied()
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        lock.contains(&entity).then_some(entity)
    }
}

impl<T: Component> Queryable for &T {
    type LockedColumns = Option<ReadLockedColumns>;
    type Item<'a> = Ref<'a, T>;

    fn reads() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn writes() -> Vec<TypeId> {
        vec![]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        let col_index = archetype.index_of(TypeId::of::<T>())?;

        let column = archetype.columns()[col_index].read();
        let ticks = archetype.ticks()[col_index].read();
        let entities: Vec<Entity> = archetype.entity_iter().collect();
        let entity_indices = entities
            .iter()
            .map(|entity| archetype.entity_index(*entity).unwrap())
            .collect();

        Some(ReadLockedColumns {
            column,
            ticks,
            entities,
            entity_indices,
        })
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_ref().map_or_else(
            || itertools::Either::Left(std::iter::empty()),
            |cols| {
                let ReadLockedColumns {
                    column,
                    ticks,
                    entities: _,
                    entity_indices,
                } = cols;
                itertools::Either::Right(entity_indices.iter().map(move |&index| {
                    let item = column.downcast_ref().unwrap().get(index).unwrap();
                    let ticks = ticks.get(index).unwrap();
                    Ref::new(item, last_run, this_run, ticks)
                }))
            },
        )
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        lock.as_mut().and_then(|cols| {
            cols.entity_indices
                .iter()
                .position(|&index| cols.entities[index] == entity)
                .map(|index| {
                    let item = cols.column.downcast_ref().unwrap().get(index).unwrap();
                    let ticks = cols.ticks.get(index).unwrap();
                    Ref::new(item, last_run, this_run, ticks)
                })
        })
    }
}

impl<T: Component> Queryable for &mut T {
    type LockedColumns = Option<WriteLockedColumns>;
    type Item<'a> = Mut<'a, T>;

    fn reads() -> Vec<TypeId> {
        vec![]
    }

    fn writes() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        let col_index = archetype.index_of(TypeId::of::<T>())?;

        let column = archetype.columns()[col_index].write();
        let ticks = archetype.ticks()[col_index].write();
        let entities: Vec<Entity> = archetype.entity_iter().collect();
        let entity_indices = entities
            .iter()
            .map(|entity| archetype.entity_index(*entity).unwrap())
            .collect();

        Some(WriteLockedColumns {
            column,
            ticks,
            entities,
            entity_indices,
        })
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        if let Some(cols) = lock {
            itertools::Either::Right(QueryableIterMut {
                column: &mut cols.column,
                ticks: &mut cols.ticks,
                entities: &cols.entities,
                entity_indices: &cols.entity_indices,
                last_run,
                this_run,
                index: 0,
                _marker: std::marker::PhantomData,
            })
        } else {
            itertools::Either::Left(std::iter::empty())
        }
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        // lock.as_mut().and_then(|(lock, ents)| {
        //     ents.iter()
        //         .position(|&e| e == entity)
        //         .map(move |index| lock.downcast_mut::<T>().unwrap().get_mut(index).unwrap())
        // })
        if let Some(cols) = lock {
            cols.entity_indices
                .iter()
                .position(|&index| cols.entities[index] == entity)
                .map(|index| {
                    let item = cols.column.downcast_mut().unwrap().get_mut(index).unwrap();
                    let ticks = cols.ticks.get_mut(index).unwrap();
                    Mut::new(item, last_run, this_run, ticks)
                })
        } else {
            None
        }
    }
}

pub struct QueryableIterMut<'a, T: Component> {
    column: &'a mut ComponentVec,
    ticks: &'a mut Vec<ComponentTicks>,
    entities: &'a [Entity],
    entity_indices: &'a [usize],
    last_run: Tick,
    this_run: Tick,
    index: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T: Component> Iterator for QueryableIterMut<'a, T> {
    type Item = Mut<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.entities.len() {
            let index = self.entity_indices[self.index];
            let item = self
                .column
                .downcast_mut::<T>()
                .unwrap()
                .get_mut(index)
                .unwrap();
            let ticks = self.ticks.get_mut(index).unwrap();
            self.index += 1;

            // SAFETY: Types and lifetimes should be valid
            let item = unsafe { &mut *(item as *mut T) };
            let ticks = unsafe { &mut *(ticks as *mut ComponentTicks) };

            Some(Mut::new(item, self.last_run, self.this_run, ticks))
        } else {
            None
        }
    }
}

impl<T: Component> Queryable for Option<&T> {
    type LockedColumns = Option<ReadLockedColumns>;
    type Item<'a> = Option<Ref<'a, T>>;

    fn reads() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn writes() -> Vec<TypeId> {
        vec![]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        <&T as Queryable>::lock_columns(archetype)
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_ref().map_or_else(
            || itertools::Either::Left(std::iter::repeat_with(|| None)),
            |cols| {
                let ReadLockedColumns {
                    column,
                    ticks,
                    entities: _,
                    entity_indices,
                } = cols;
                itertools::Either::Right(entity_indices.iter().map(move |&index| {
                    let item = column.downcast_ref().unwrap().get(index).unwrap();
                    let ticks = ticks.get(index).unwrap();
                    Some(Ref::new(item, last_run, this_run, ticks))
                }))
            },
        )
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        lock.as_mut().and_then(|cols| {
            cols.entity_indices
                .iter()
                .position(|&index| cols.entities[index] == entity)
                .map(|index| {
                    let item = cols.column.downcast_ref().unwrap().get(index).unwrap();
                    let ticks = cols.ticks.get(index).unwrap();
                    Some(Ref::new(item, last_run, this_run, ticks))
                })
        })
    }
}

impl<T: Component> Queryable for Option<&mut T> {
    type LockedColumns = Option<WriteLockedColumns>;
    type Item<'a> = Option<Mut<'a, T>>;

    fn reads() -> Vec<TypeId> {
        vec![]
    }

    fn writes() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        <&mut T as Queryable>::lock_columns(archetype)
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_mut().map_or_else(
            || itertools::Either::Left(std::iter::repeat_with(|| None)),
            |cols| {
                itertools::Either::Right(
                    QueryableIterMut {
                        column: &mut cols.column,
                        ticks: &mut cols.ticks,
                        entities: &cols.entities,
                        entity_indices: &cols.entity_indices,
                        last_run,
                        this_run,
                        index: 0,
                        _marker: std::marker::PhantomData,
                    }
                    .map(Some),
                )
            },
        )
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        if let Some(cols) = lock {
            cols.entity_indices
                .iter()
                .position(|&index| cols.entities[index] == entity)
                .map(|index| {
                    let item = cols.column.downcast_mut().unwrap().get_mut(index).unwrap();
                    let ticks = cols.ticks.get_mut(index).unwrap();
                    Some(Mut::new(item, last_run, this_run, ticks))
                })
        } else {
            None
        }
    }
}

pub struct With<T: Component>(std::marker::PhantomData<T>);

impl<T: Component> Queryable for With<T> {
    type LockedColumns = Option<Vec<Entity>>;
    type Item<'a> = &'a ();

    fn reads() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn writes() -> Vec<TypeId> {
        vec![]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        archetype.index_of(TypeId::of::<T>())?; // check if the component is in the archetype
        Some(archetype.entity_iter().collect())
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        _last_run: Tick,
        _this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_ref().map_or_else(
            || itertools::Either::Left(std::iter::empty()),
            |entities| itertools::Either::Right(entities.iter().map(|_| &())),
        )
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        lock.as_ref()
            .and_then(|entities| entities.iter().find(|&&e| e == entity).map(|_| &()))
    }
}

pub struct Without<T: Component>(std::marker::PhantomData<T>);

pub struct Changed<T>(std::marker::PhantomData<T>);

impl<T: Component> Queryable for Changed<&T> {
    type LockedColumns = Option<ReadLockedColumns>;
    type Item<'a> = Ref<'a, T>;

    fn reads() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn writes() -> Vec<TypeId> {
        vec![]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        let col_index = archetype.index_of(TypeId::of::<T>())?;

        let column = archetype.columns()[col_index].read();
        let ticks = archetype.ticks()[col_index].read();
        let entities: Vec<Entity> = archetype.entity_iter().collect();
        let entity_indices = entities
            .iter()
            .map(|entity| archetype.entity_index(*entity).unwrap())
            .collect();

        Some(ReadLockedColumns {
            column,
            ticks,
            entities,
            entity_indices,
        })
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_ref().map_or_else(
            || itertools::Either::Left(std::iter::empty()),
            |cols| {
                let ReadLockedColumns {
                    column,
                    ticks,
                    entities: _,
                    entity_indices,
                } = cols;
                itertools::Either::Right(entity_indices.iter().filter_map(move |&index| {
                    let item = column.downcast_ref().unwrap().get(index).unwrap();
                    let ticks = ticks.get(index).unwrap();
                    if ticks.is_changed(last_run, this_run) {
                        Some(Ref::new(item, last_run, this_run, ticks))
                    } else {
                        None
                    }
                }))
            },
        )
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        lock.as_mut().and_then(|cols| {
            cols.entity_indices
                .iter()
                .position(|&index| cols.entities[index] == entity)
                .and_then(|index| {
                    let item = cols.column.downcast_ref().unwrap().get(index).unwrap();
                    let ticks = cols.ticks.get(index).unwrap();
                    if ticks.is_changed(last_run, this_run) {
                        Some(Ref::new(item, last_run, this_run, ticks))
                    } else {
                        None
                    }
                })
        })
    }
}

impl<T: Component> Queryable for Changed<&mut T> {
    type LockedColumns = Option<WriteLockedColumns>;
    type Item<'a> = Mut<'a, T>;

    fn reads() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn writes() -> Vec<TypeId> {
        vec![]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        let col_index = archetype.index_of(TypeId::of::<T>())?;

        let column = archetype.columns()[col_index].write();
        let ticks = archetype.ticks()[col_index].write();
        let entities: Vec<Entity> = archetype.entity_iter().collect();
        let entity_indices = entities
            .iter()
            .map(|entity| archetype.entity_index(*entity).unwrap())
            .collect();

        Some(WriteLockedColumns {
            column,
            ticks,
            entities,
            entity_indices,
        })
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        if let Some(cols) = lock {
            itertools::Either::Right(
                QueryableIterMut {
                    column: &mut cols.column,
                    ticks: &mut cols.ticks,
                    entities: &cols.entities,
                    entity_indices: &cols.entity_indices,
                    last_run,
                    this_run,
                    index: 0,
                    _marker: std::marker::PhantomData,
                }
                .filter_map(move |item: Mut<'a, T>| {
                    if item.get_ticks().is_changed(last_run, this_run) {
                        Some(item)
                    } else {
                        None
                    }
                }),
            )
        } else {
            itertools::Either::Left(std::iter::empty())
        }
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        if let Some(cols) = lock {
            cols.entity_indices
                .iter()
                .position(|&index| cols.entities[index] == entity)
                .and_then(|index| {
                    let item = cols.column.downcast_mut().unwrap().get_mut(index).unwrap();
                    let ticks = cols.ticks.get_mut(index).unwrap();
                    if ticks.is_changed(last_run, this_run) {
                        Some(Mut::new(item, last_run, this_run, ticks))
                    } else {
                        None
                    }
                })
        } else {
            None
        }
    }
}

macro_rules! impl_queryable_tuple {
    ($( $name:ident ),*) => {
        #[allow(non_snake_case)]
        impl<$($name: Queryable),*> Queryable for ($($name,)*) {
            type LockedColumns = ($($name::LockedColumns,)*);
            type Item<'a> = ($($name::Item<'a>,)*);

            fn reads() -> Vec<TypeId> {
                let mut reads = Vec::new();
                $(reads.extend($name::reads());)*
                reads
            }

            fn writes() -> Vec<TypeId> {
                let mut writes = Vec::new();
                $(writes.extend($name::writes());)*
                writes
            }

            fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
                ($($name::lock_columns(archetype),)*)
            }

            fn iter_mut<'a, 'b: 'a>(lock: &'b mut Self::LockedColumns, last_run: Tick, this_run: Tick) -> impl Iterator<Item = Self::Item<'a>> {
                let ($($name,)*) = lock;
                itertools::izip!($( $name::iter_mut($name, last_run, this_run), )*)
            }

            fn get<'a, 'b: 'a>(entity: Entity, lock: &'b mut Self::LockedColumns, last_run: Tick, this_run: Tick) -> Option<Self::Item<'a>> {
                let ($($name,)*) = lock;
                Some(($( $name::get(entity, $name, last_run, this_run)?, )*))
            }
        }
    };
}

impl<A: Queryable> Queryable for (A,) {
    type LockedColumns = (A::LockedColumns,);
    type Item<'a> = (A::Item<'a>,);

    fn reads() -> Vec<TypeId> {
        A::reads()
    }

    fn writes() -> Vec<TypeId> {
        A::writes()
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        (A::lock_columns(archetype),)
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        let (a,) = lock;
        A::iter_mut(a, last_run, this_run).map(|a| (a,))
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        let (a,) = lock;
        A::get(entity, a, last_run, this_run).map(|a| (a,))
    }
}

impl_queryable_tuple!(A, B);
impl_queryable_tuple!(A, B, C);
impl_queryable_tuple!(A, B, C, D);
impl_queryable_tuple!(A, B, C, D, E);
impl_queryable_tuple!(A, B, C, D, E, F);
impl_queryable_tuple!(A, B, C, D, E, F, G);
impl_queryable_tuple!(A, B, C, D, E, F, G, H);

pub struct Query<Q: Queryable> {
    locked_columns: Vec<Q::LockedColumns>,
    last_run: Tick,
    this_run: Tick,
}

impl<Q: Queryable> Query<Q> {
    pub fn new(world: &World) -> Self {
        let locked_columns: Vec<Q::LockedColumns> = world
            .components()
            .archetype_iter()
            .map(Q::lock_columns)
            .collect();
        let last_run = world.last_change_tick();
        let this_run = world.read_change_tick();
        Self {
            locked_columns,
            last_run,
            this_run,
        }
    }

    pub fn iter(&mut self) -> impl Iterator<Item = Q::Item<'_>> + '_ {
        self.locked_columns
            .iter_mut()
            .flat_map(|cols| Q::iter_mut(cols, self.last_run, self.this_run))
    }

    pub fn get(&mut self, entity: Entity) -> Option<Q::Item<'_>> {
        self.locked_columns
            .iter_mut()
            .find_map(|lock| Q::get(entity, lock, self.last_run, self.this_run))
    }
}

impl<Q: Queryable> SystemParam for Query<Q> {
    type Item = Query<Q>;
    type State = ();

    fn access() -> SystemAccess {
        let reads = Q::reads();
        let writes = Q::writes();
        SystemAccess {
            resources_read: FxHashSet::from_iter([TypeId::of::<Components>()]),
            components_read: FxHashSet::from_iter(reads),
            components_written: FxHashSet::from_iter(writes),
            ..SystemAccess::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _state: &Self::State) -> Self::Item {
        Query::new(world)
    }
}
