use std::any::TypeId;

use atomic_refcell::{AtomicRef, AtomicRefMut};

use crate::{
    archetype::Archetype,
    storage::{ComponentStorage, Components, EntitySet},
};

use super::{entity::EntityId, storage::ComponentSet, Bundle, Component};

pub struct Ref<'a, T>
where
    T: Component,
{
    component: AtomicRef<'a, T>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T> std::ops::Deref for Ref<'a, T>
where
    T: Component,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.component
    }
}

pub struct Mut<'a, T>
where
    T: Component,
{
    component: AtomicRefMut<'a, T>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T> std::ops::Deref for Mut<'a, T>
where
    T: Component,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.component
    }
}

impl<'a, T> std::ops::DerefMut for Mut<'a, T>
where
    T: Component,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.component
    }
}

#[derive(Clone, Default)]
pub struct QueryAccess {
    pub reads: ComponentSet,
    pub writes: ComponentSet,
    pub withs: ComponentSet,
    pub withouts: ComponentSet,
}

impl QueryAccess {
    pub fn matches_archetype(&self, archetype: &Archetype) -> bool {
        let mut includes = ComponentSet::default();

        includes.extend(&self.reads);
        includes.extend(&self.writes);
        includes.extend(&self.withs);

        if !self.withouts.is_empty() && self.withouts.is_subset(&archetype.components) {
            return false;
        }

        includes.is_subset(&archetype.components)
    }
}

pub trait Queryable<'a, F = ()>
where
    F: QueryFilter<'a>,
{
    type Item: Bundle;
    type ItemRef: 'a + Send;

    fn fetch(components: &'a ComponentStorage) -> Option<Self::ItemRef>;

    fn map_iter<Gen, I>(gen_base: &Gen) -> Box<dyn Iterator<Item = Self::ItemRef> + 'a>
    where
        Gen: Fn() -> I + 'a,
        I: Iterator<Item = &'a ComponentStorage> + 'a;

    fn access() -> QueryAccess;
}

impl<'a, F> Queryable<'a, F> for ()
where
    F: QueryFilter<'a>,
{
    type Item = ();
    type ItemRef = ();

    fn fetch(_components: &ComponentStorage) -> Option<Self::ItemRef> {
        Some(())
    }

    fn map_iter<Gen, I>(gen_base: &Gen) -> Box<dyn Iterator<Item = Self::ItemRef> + 'a>
    where
        Gen: Fn() -> I + 'a,
        I: Iterator<Item = &'a ComponentStorage> + 'a,
    {
        Box::new(gen_base().map(|_| ()))
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::default(),
            withs: F::withs(),
            withouts: F::withouts(),
        }
    }
}

impl<'a, T, F> Queryable<'a, F> for &'a T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = Ref<'a, T>;

    fn fetch(components: &'a ComponentStorage) -> Option<Self::ItemRef> {
        let component = components.get(&TypeId::of::<Self::Item>())?;
        Some(Ref {
            component: AtomicRef::map(component.component.borrow(), |component| {
                (*component).as_any().downcast_ref::<T>().unwrap()
            }),
            _marker: std::marker::PhantomData,
        })
    }

    fn map_iter<Gen, I>(gen_base: &Gen) -> Box<dyn Iterator<Item = Self::ItemRef> + 'a>
    where
        Gen: Fn() -> I + 'a,
        I: Iterator<Item = &'a ComponentStorage> + 'a,
    {
        Box::new(gen_base().filter_map(|c| {
            c.components.dense_iter().find_map(|c| {
                if c.component_id == TypeId::of::<T>() {
                    Some(Ref {
                        component: AtomicRef::map(c.component.borrow(), |component| {
                            (*component).as_any().downcast_ref::<T>().unwrap()
                        }),
                        _marker: std::marker::PhantomData,
                    })
                } else {
                    None
                }
            })
        }))
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::from_iter([TypeId::of::<T>()]),
            writes: ComponentSet::default(),
            withs: F::withs(),
            withouts: F::withouts(),
        }
    }
}

impl<'a, T, F> Queryable<'a, F> for &'a mut T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = Mut<'a, T>;

    fn fetch(components: &'a ComponentStorage) -> Option<Self::ItemRef> {
        let component = components.get(&TypeId::of::<Self::Item>())?;
        Some(Mut {
            component: AtomicRefMut::map(component.component.borrow_mut(), |component| {
                (*component).as_any_mut().downcast_mut::<T>().unwrap()
            }),
            _marker: std::marker::PhantomData,
        })
    }

    fn map_iter<Gen, I>(gen_base: &Gen) -> Box<dyn Iterator<Item = Self::ItemRef> + 'a>
    where
        Gen: Fn() -> I + 'a,
        I: Iterator<Item = &'a ComponentStorage> + 'a,
    {
        Box::new(gen_base().filter_map(|c| {
            c.components.dense_iter().find_map(|c| {
                if c.component_id == TypeId::of::<T>() {
                    Some(Mut {
                        component: AtomicRefMut::map(c.component.borrow_mut(), |component| {
                            (*component).as_any_mut().downcast_mut::<T>().unwrap()
                        }),
                        _marker: std::marker::PhantomData,
                    })
                } else {
                    None
                }
            })
        }))
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::from_iter([TypeId::of::<T>()]),
            withs: F::withs(),
            withouts: F::withouts(),
        }
    }
}

/// Very similar to a Queryable, but instead of yielding a reference to the component, it is just used for filtering.
pub trait QueryFilter<'a> {
    fn withs() -> ComponentSet {
        ComponentSet::default()
    }
    fn withouts() -> ComponentSet {
        ComponentSet::default()
    }
}

/// Default pass-through filter that yields all entries.
impl<'a> QueryFilter<'a> for () {}

pub struct With<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for With<'a, T>
where
    T: Component,
{
    fn withs() -> ComponentSet {
        ComponentSet::from_iter([TypeId::of::<T>()])
    }
}

pub struct Without<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for Without<'a, T>
where
    T: Component,
{
    fn withouts() -> ComponentSet {
        ComponentSet::from_iter([TypeId::of::<T>()])
    }
}

pub struct QueryState<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    entities: EntitySet,
    _filter: std::marker::PhantomData<&'a (Q, F)>,
}

impl<'a, Q, F> QueryState<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub fn new(components: &Components) -> Self {
        let entities = components.entities_matching_access(&Q::access());

        QueryState {
            entities,
            _filter: std::marker::PhantomData,
        }
    }

    pub fn get(&self, entity: EntityId, components: &'a Components) -> Option<Q::ItemRef> {
        if self.entities.contains(&entity) {
            let components = components.entity_components.get(&entity)?;
            Q::fetch(components)
        } else {
            None
        }
    }

    pub fn iter(&'a self, components: &'a Components) -> QueryIter<'a, Q, F> {
        let iter = || {
            components
                .entity_components
                .dense_iter()
                .filter(|c| self.entities.contains(&c.entity))
        };

        let iter = Q::map_iter(&iter);

        QueryIter {
            iter: Box::new(iter),
            _filter: std::marker::PhantomData,
        }
    }
}

pub struct Query<'a, Q, F = ()>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    components: &'a Components,
    state: QueryState<'a, Q, F>,
}

impl<'a, Q, F> Query<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub fn new(components: &'a Components) -> Self {
        Query {
            components,
            state: QueryState::new(components),
        }
    }

    pub fn get(&self, entity: EntityId) -> Option<Q::ItemRef> {
        self.state.get(entity, self.components)
    }

    pub fn iter(&'a self) -> QueryIter<'a, Q, F> {
        self.state.iter(self.components)
    }
}

pub struct QueryIter<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    iter: Box<dyn Iterator<Item = Q::ItemRef> + 'a>,
    _filter: std::marker::PhantomData<(Q, F)>,
}

impl<'a, Q, F> Iterator for QueryIter<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    type Item = Q::ItemRef;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

macro_rules! impl_queryable_for_tuple {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<'a, $($name),*, F> Queryable<'a, F> for ($($name,)*)
        where
            $($name: Queryable<'a>,)*
            F: QueryFilter<'a>,
        {
            type Item = ($($name::Item,)*);
            type ItemRef = ($($name::ItemRef,)*);

            fn fetch(component: &'a ComponentStorage) -> Option<Self::ItemRef> {
                let ($($name,)*) = ($($name::fetch(component)?,)*);
                Some(($($name,)*))
            }

            fn map_iter<Gen, I>(gen_base: &Gen) -> Box<dyn Iterator<Item = Self::ItemRef> + 'a>
            where
                Gen: Fn() -> I + 'a,
                I: Iterator<Item = &'a ComponentStorage> + 'a,
            {

                $(
                    let $name = $name::map_iter(gen_base);
                )*

                Box::new(itertools::multizip(($($name,)*)))
            }


            fn access() -> QueryAccess {
                let mut reads = ComponentSet::default();
                let mut writes = ComponentSet::default();
                let mut withs = ComponentSet::default();
                let mut withouts = ComponentSet::default();

                $({
                    let access = $name::access();
                    reads.extend(&access.reads);
                    writes.extend(&access.writes);
                    withs.extend(&access.withs);
                    withouts.extend(&access.withouts);
                })*

                QueryAccess {
                    reads,
                    writes,
                    withs,
                    withouts,
                }
            }
        }
    };
}

impl_queryable_for_tuple!(A);
impl_queryable_for_tuple!(A, B);
impl_queryable_for_tuple!(A, B, C);
impl_queryable_for_tuple!(A, B, C, D);

macro_rules! impl_queryfilter_for_tuple {
    ($($name:ident),*) => {
        impl<'a, $($name),*> QueryFilter<'a> for ($($name,)*)
        where
            $($name: QueryFilter<'a>,)*
        {
            fn withs() -> ComponentSet {
                let mut all = ComponentSet::default();
                $(
                    all.extend(&$name::withs());
                )*
                all
            }

            fn withouts() -> ComponentSet {
                let mut all = ComponentSet::default();
                $(
                    all.extend(&$name::withouts());
                )*
                all
            }
        }
    };
}

impl_queryfilter_for_tuple!(A);
impl_queryfilter_for_tuple!(A, B);
impl_queryfilter_for_tuple!(A, B, C);
impl_queryfilter_for_tuple!(A, B, C, D);
