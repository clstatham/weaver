use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{BTreeMap, BTreeSet},
    ops::BitAnd,
    sync::{RwLockReadGuard, RwLockWriteGuard},
};

use crate::{Component, Entity, World};

pub trait Queryable<'w, 'q, 'i>
where
    'w: 'q,
    'q: 'i,
{
    type Item;
    type ItemRef: 'i;
    type Iter: Iterator<Item = Self::ItemRef> + 'i;

    fn create(world: &'w World) -> Self;
    fn entities(&self) -> BTreeSet<Entity>;
    fn get(&'q self, entity: Entity) -> Option<Self::ItemRef>;
    fn iter(&'q self) -> Self::Iter;

    fn components_read() -> Vec<u64>
    where
        Self: Sized;
    fn components_written() -> Vec<u64>
    where
        Self: Sized;
}

pub struct Read<'a, T> {
    entries: BTreeMap<Entity, RefCell<RwLockReadGuard<'a, dyn Component>>>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T> Read<'a, T>
where
    T: Component,
{
    pub(crate) fn new(world: &'a crate::World) -> Self {
        let entries = world.entities_components.iter().fold(
            BTreeMap::new(),
            |mut entries, (&entity, components)| {
                if let Some(component) = components.get(&T::component_id()) {
                    match world.borrow_intent.try_read::<T>(entity) {
                        Some(Ok(())) => {
                            entries.insert(entity, RefCell::new(component.read().unwrap()));
                        }
                        Some(Err(e)) => {
                            panic!("Failed to borrow component: {:?}", e);
                        }
                        None => {
                            // component doesn't exist for the entity; keep going
                        }
                    }
                }
                entries
            },
        );
        Self {
            entries,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<'w, 'q, 'i, T> Queryable<'w, 'q, 'i> for Read<'w, T>
where
    'w: 'q,
    'q: 'i,
    T: Component,
{
    type Item = T;
    type ItemRef = Ref<'i, T>;
    type Iter = Box<dyn Iterator<Item = Self::ItemRef> + 'i>;

    fn create(world: &'w World) -> Self {
        Self::new(world)
    }

    fn entities(&self) -> BTreeSet<Entity> {
        self.entries.keys().copied().collect()
    }

    fn components_read() -> Vec<u64>
    where
        Self: Sized,
    {
        vec![T::component_id()]
    }

    fn components_written() -> Vec<u64>
    where
        Self: Sized,
    {
        vec![]
    }

    fn get(&'q self, entity: Entity) -> Option<Self::ItemRef> {
        self.entries
            .get(&entity)
            .filter(|component| component.borrow().as_any().is::<T>())
            .map(|component| {
                Ref::map(component.borrow(), |component| {
                    component
                        .as_any()
                        .downcast_ref::<T>()
                        .expect("Failed to downcast component")
                })
            })
    }

    fn iter(&'q self) -> Self::Iter {
        Box::new(self.entries.values().map(|component| {
            Ref::map(component.borrow(), |component| {
                component
                    .as_any()
                    .downcast_ref::<T>()
                    .expect("Failed to downcast component")
            })
        }))
    }
}

pub struct Write<'a, T> {
    entries: BTreeMap<Entity, RefCell<RwLockWriteGuard<'a, dyn Component>>>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T> Write<'a, T>
where
    T: Component,
{
    pub(crate) fn new(world: &'a crate::World) -> Self {
        let entries = world.entities_components.iter().fold(
            BTreeMap::new(),
            |mut entries, (&entity, components)| {
                if let Some(component) = components.get(&T::component_id()) {
                    match world.borrow_intent.try_write::<T>(entity) {
                        Some(Ok(())) => {
                            entries.insert(entity, RefCell::new(component.write().unwrap()));
                        }
                        Some(Err(e)) => {
                            panic!("Failed to borrow component: {:?}", e);
                        }
                        None => {
                            // component doesn't exist for the entity; keep going
                        }
                    }
                }
                entries
            },
        );
        Self {
            entries,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<'w, 'q, 'i, T> Queryable<'w, 'q, 'i> for Write<'w, T>
where
    'w: 'q,
    'q: 'i,
    T: Component,
{
    type Item = T;
    type ItemRef = RefMut<'i, T>;
    type Iter = Box<dyn Iterator<Item = Self::ItemRef> + 'i>;

    fn create(world: &'w World) -> Self {
        Self::new(world)
    }

    fn entities(&self) -> BTreeSet<Entity> {
        self.entries.keys().copied().collect()
    }

    fn components_read() -> Vec<u64>
    where
        Self: Sized,
    {
        vec![]
    }

    fn components_written() -> Vec<u64>
    where
        Self: Sized,
    {
        vec![T::component_id()]
    }

    fn get(&'q self, entity: Entity) -> Option<Self::ItemRef> {
        self.entries
            .get(&entity)
            .filter(|component| component.borrow().as_any().is::<T>())
            .map(|component| {
                RefMut::map(component.borrow_mut(), |component| {
                    component
                        .as_any_mut()
                        .downcast_mut::<T>()
                        .expect("Failed to downcast component")
                })
            })
    }

    fn iter(&'q self) -> Self::Iter {
        Box::new(self.entries.values().map(|component| {
            RefMut::map(component.borrow_mut(), |component| {
                component
                    .as_any_mut()
                    .downcast_mut::<T>()
                    .expect("Failed to downcast component")
            })
        }))
    }
}

weaver_proc_macro::impl_queryable_for_n_tuple!(2);
weaver_proc_macro::impl_queryable_for_n_tuple!(3);
weaver_proc_macro::impl_queryable_for_n_tuple!(4);

pub struct Query<'w, 'q, 'i, T>
where
    'w: 'q,
    'q: 'i,
    T: Queryable<'w, 'q, 'i>,
{
    query: T,
    _marker: std::marker::PhantomData<(&'w (), &'q (), &'i ())>,
}

impl<'w, 'q, 'i, T> Queryable<'w, 'q, 'i> for Query<'w, 'q, 'i, T>
where
    'w: 'q,
    'q: 'i,
    T: Queryable<'w, 'q, 'i>,
{
    type Item = T::Item;
    type ItemRef = T::ItemRef;
    type Iter = T::Iter;

    fn create(world: &'w World) -> Self {
        Self {
            query: T::create(world),
            _marker: std::marker::PhantomData,
        }
    }

    fn entities(&self) -> BTreeSet<Entity> {
        self.query.entities()
    }

    fn components_read() -> Vec<u64>
    where
        Self: Sized,
    {
        T::components_read()
    }

    fn components_written() -> Vec<u64>
    where
        Self: Sized,
    {
        T::components_written()
    }

    fn get(&'q self, entity: Entity) -> Option<Self::ItemRef> {
        self.query.get(entity)
    }

    fn iter(&'q self) -> Self::Iter {
        self.query.iter()
    }
}
