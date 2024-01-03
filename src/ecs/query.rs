use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{BTreeMap, BTreeSet},
    ops::BitAnd,
    sync::{RwLockReadGuard, RwLockWriteGuard},
};

use super::{world::Components, Component, Entity};

pub trait Queryable<'w, 'q, 'i>
where
    'w: 'q,
    'q: 'i,
{
    type Item;
    type ItemRef: 'i;
    type Iter: Iterator<Item = Self::ItemRef> + 'i;

    fn create(entities_components: &'w Components) -> Self;
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
    pub(crate) fn new(entities_components: &'a Components) -> Self {
        let entries = entities_components.iter().fold(
            BTreeMap::new(),
            |mut entries, (&entity, components)| {
                if let Some(component) = components.get(&T::component_id()) {
                    entries.insert(
                        entity,
                        RefCell::new(
                            component
                                .try_read()
                                .expect("BUG: Failed to lock component for reading"),
                        ),
                    );
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

    fn create(entities_components: &'w Components) -> Self {
        Self::new(entities_components)
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
                        .expect("BUG: Failed to downcast component")
                })
            })
    }

    fn iter(&'q self) -> Self::Iter {
        Box::new(self.entries.values().map(|component| {
            Ref::map(component.borrow(), |component| {
                component
                    .as_any()
                    .downcast_ref::<T>()
                    .expect("BUG: Failed to downcast component")
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
    pub(crate) fn new(entities_components: &'a Components) -> Self {
        let entries = entities_components.iter().fold(
            BTreeMap::new(),
            |mut entries, (&entity, components)| {
                if let Some(component) = components.get(&T::component_id()) {
                    entries.insert(
                        entity,
                        RefCell::new(
                            component
                                .try_write()
                                .expect("BUG: Failed to lock component for writing"),
                        ),
                    );
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

    fn create(entities_components: &'w Components) -> Self {
        Self::new(entities_components)
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
                        .expect("BUG: Failed to downcast component")
                })
            })
    }

    fn iter(&'q self) -> Self::Iter {
        Box::new(self.entries.values().map(|component| {
            RefMut::map(component.borrow_mut(), |component| {
                component
                    .as_any_mut()
                    .downcast_mut::<T>()
                    .expect("BUG: Failed to downcast component")
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

    fn create(entities_components: &'w Components) -> Self {
        Self {
            query: T::create(entities_components),
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
