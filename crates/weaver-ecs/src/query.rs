use std::any::TypeId;

use weaver_util::prelude::*;

use crate::{
    entity::Entity,
    loan::{Loan, LoanMut},
    prelude::{Archetype, Component, ComponentVec, SystemAccess, SystemParam},
    storage::Components,
};

use super::world::World;

pub type ReadLockedColumns = Option<(Loan<ComponentVec>, Vec<Entity>)>;
pub type WriteLockedColumns = Option<(LoanMut<ComponentVec>, Vec<Entity>)>;

pub trait Queryable: Send + Sync {
    type LockedColumns: Send + Sync;
    type Item<'a>: Send + Sync;

    fn reads() -> Vec<TypeId>;

    fn writes() -> Vec<TypeId>;

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns;

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
    ) -> impl Iterator<Item = Self::Item<'a>>;

    fn get<'a, 'b: 'a>(entity: Entity, lock: &'b mut Self::LockedColumns)
        -> Option<Self::Item<'a>>;
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
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.iter().copied()
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
    ) -> Option<Self::Item<'a>> {
        lock.contains(&entity).then_some(entity)
    }
}

impl<'s, T: Component> Queryable for &'s T {
    type LockedColumns = ReadLockedColumns;
    type Item<'a> = &'a T;

    fn reads() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn writes() -> Vec<TypeId> {
        vec![]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        let col_index = archetype.index_of(TypeId::of::<T>())?;
        Some((
            archetype.columns()[col_index]
                .write()
                .loan()
                .unwrap_or_else(|| panic!("Failed to lock column of type {}", T::type_name())),
            archetype.entity_iter().collect(),
        ))
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_ref().map_or_else(
            || [].iter(),
            |(lock, _)| lock.downcast_ref::<T>().unwrap().into_iter(),
        )
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
    ) -> Option<Self::Item<'a>> {
        lock.as_mut().and_then(|(lock, ents)| {
            ents.iter()
                .position(|&e| e == entity)
                .map(move |index| lock.downcast_ref::<T>().unwrap().get(index).unwrap())
        })
    }
}

impl<'s, T: Component> Queryable for &'s mut T {
    type LockedColumns = WriteLockedColumns;
    type Item<'a> = &'a mut T;

    fn reads() -> Vec<TypeId> {
        vec![]
    }

    fn writes() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn lock_columns(archetype: &Archetype) -> Self::LockedColumns {
        let col_index = archetype.index_of(TypeId::of::<T>())?;
        Some((
            archetype.columns()[col_index]
                .write()
                .loan_mut()
                .unwrap_or_else(|| {
                    panic!("Failed to mutably lock column of type {}", T::type_name())
                }),
            archetype.entity_iter().collect(),
        ))
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_mut().map_or_else(
            || [].iter_mut(),
            |(lock, _)| lock.downcast_mut::<T>().unwrap().into_iter(),
        )
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
    ) -> Option<Self::Item<'a>> {
        lock.as_mut().and_then(|(lock, ents)| {
            ents.iter()
                .position(|&e| e == entity)
                .map(move |index| lock.downcast_mut::<T>().unwrap().get_mut(index).unwrap())
        })
    }
}

impl<'s, T: Component> Queryable for Option<&'s T> {
    type LockedColumns = ReadLockedColumns;
    type Item<'a> = Option<&'a T>;

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
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_ref().map_or_else(
            || itertools::Either::Left(std::iter::repeat_with(|| None)),
            |(lock, _)| {
                itertools::Either::Right(lock.downcast_ref::<T>().unwrap().into_iter().map(Some))
            },
        )
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
    ) -> Option<Self::Item<'a>> {
        lock.as_ref().and_then(|(lock, ents)| {
            ents.iter().position(|&e| e == entity).map(move |index| {
                lock.downcast_ref::<T>()
                    .unwrap()
                    .get(index)
                    .map(Some)
                    .unwrap_or(None)
            })
        })
    }
}

impl<'s, T: Component> Queryable for Option<&'s mut T> {
    type LockedColumns = WriteLockedColumns;
    type Item<'a> = Option<&'a mut T>;

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
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_mut().map_or_else(
            || itertools::Either::Left(std::iter::repeat_with(|| None)),
            |(lock, _)| {
                itertools::Either::Right(lock.downcast_mut::<T>().unwrap().into_iter().map(Some))
            },
        )
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
    ) -> Option<Self::Item<'a>> {
        lock.as_mut().and_then(|(lock, ents)| {
            ents.iter().position(|&e| e == entity).map(move |index| {
                lock.downcast_mut::<T>()
                    .unwrap()
                    .get_mut(index)
                    .map(Some)
                    .unwrap_or(None)
            })
        })
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

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
    ) -> Option<Self::Item<'a>> {
        lock.as_ref()
            .and_then(|ents| ents.contains(&entity).then_some(&()))
    }

    fn iter_mut<'a, 'b: 'a>(
        lock: &'b mut Self::LockedColumns,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        lock.as_ref().map_or_else(
            || itertools::Either::Left([].iter()),
            |lock| itertools::Either::Right(lock.iter().map(|_| &())),
        )
    }
}

macro_rules! impl_queryable_tuple {
    ($( $name:ident ),*) => {
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

            fn iter_mut<'a, 'b: 'a>(lock: &'b mut Self::LockedColumns) -> impl Iterator<Item = Self::Item<'a>> {
                let ($(ref mut $name,)*) = lock;
                itertools::izip!($( $name::iter_mut($name), )*)
            }

            fn get<'a, 'b: 'a>(entity: Entity, lock: &'b mut Self::LockedColumns) -> Option<Self::Item<'a>> {
                let ($(ref mut $name,)*) = lock;
                Some(($( $name::get(entity, $name)?, )*))
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
    ) -> impl Iterator<Item = Self::Item<'a>> {
        let (ref mut a,) = lock;
        A::iter_mut(a).map(|a| (a,))
    }

    fn get<'a, 'b: 'a>(
        entity: Entity,
        lock: &'b mut Self::LockedColumns,
    ) -> Option<Self::Item<'a>> {
        let (ref mut a,) = lock;
        A::get(entity, a).map(|a| (a,))
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
}

impl<Q: Queryable> Query<Q> {
    pub fn new(world: &World) -> Self {
        let locked_columns: Vec<Q::LockedColumns> = world
            .components()
            .archetype_iter()
            .map(Q::lock_columns)
            .collect();
        Self { locked_columns }
    }

    pub fn iter(&mut self) -> impl Iterator<Item = Q::Item<'_>> + '_ {
        self.locked_columns.iter_mut().flat_map(Q::iter_mut)
    }

    pub fn get(&mut self, entity: Entity) -> Option<Q::Item<'_>> {
        self.locked_columns
            .iter_mut()
            .find_map(|lock| Q::get(entity, lock))
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
