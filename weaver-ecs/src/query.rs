use atomic_refcell::{AtomicRef, AtomicRefMut};
use std::{fmt::Debug, ops::Deref};
use weaver_proc_macro::all_tuples;

use crate::{
    component::{Data, LockedData},
    id::{DynamicId, Registry},
    prelude::Entity,
    storage::{Archetype, ComponentMap, ComponentSet, Components, SparseSet},
};

use super::{bundle::Bundle, component::Component};

pub struct Ref<'a, T>
where
    T: Component,
{
    entity: Entity,
    component: AtomicRef<'a, T>,
}

impl<'a, T> Ref<'a, T>
where
    T: Component,
{
    pub fn entity(&self) -> Entity {
        self.entity
    }
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
    entity: Entity,
    component: AtomicRefMut<'a, T>,
}

impl<'a, T> Mut<'a, T>
where
    T: Component,
{
    pub fn entity(&self) -> Entity {
        self.entity
    }
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

#[derive(Default)]
pub struct QueryAccess {
    pub reads: ComponentSet,
    pub writes: ComponentSet,
    pub withs: ComponentSet,
    pub withouts: ComponentSet,
}

impl QueryAccess {
    pub fn matches_archetype(&self, archetype: &Archetype) -> bool {
        if !self.withouts.is_empty()
            && self
                .withouts
                .intersection(&archetype.component_ids())
                .count()
                > 0
        {
            return false;
        }

        if !self.withs.is_empty()
            && self.withs.intersection(&archetype.component_ids()).count() != self.withs.len()
        {
            return false;
        }

        if !self.reads.is_empty()
            && self.reads.intersection(&archetype.component_ids()).count() != self.reads.len()
        {
            return false;
        }

        if !self.writes.is_empty()
            && self.writes.intersection(&archetype.component_ids()).count() != self.writes.len()
        {
            return false;
        }

        true
    }
}

impl Debug for QueryAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryAccess")
            .field("reads", &self.reads.iter().collect::<Vec<_>>())
            .field("writes", &self.writes.iter().collect::<Vec<_>>())
            .field("withs", &self.withs.iter().collect::<Vec<_>>())
            .field("withouts", &self.withouts.iter().collect::<Vec<_>>())
            .finish()
    }
}

pub trait Queryable<'a, F = ()>
where
    F: QueryFilter<'a>,
{
    type Item: Bundle;
    type ItemRef: 'a + Send;

    fn fetch(
        entity: Entity,
        data: &'a ComponentMap<&'a LockedData>,
        registry: &'a Registry,
    ) -> Option<Self::ItemRef>;

    fn access(registry: &Registry) -> QueryAccess;
}

impl<'a, F> Queryable<'a, F> for ()
where
    F: QueryFilter<'a>,
{
    type Item = ();
    type ItemRef = ();

    fn fetch(
        _entity: Entity,
        _data: &'a ComponentMap<&'a LockedData>,
        _registry: &'a Registry,
    ) -> Option<Self::ItemRef> {
        Some(())
    }

    fn access(registry: &Registry) -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::default(),
            withs: F::withs(registry),
            withouts: F::withouts(registry),
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

    fn fetch(
        entity: Entity,
        data: &'a ComponentMap<&'a LockedData>,
        registry: &Registry,
    ) -> Option<Self::ItemRef> {
        let data = data.get(&registry.get_static::<T>())?;
        let data = data.borrow();

        Some(Ref {
            entity,
            component: AtomicRef::map(data, |data| data.get_as::<T>().unwrap()),
        })
    }

    fn access(registry: &Registry) -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::from_iter([registry.get_static::<T>()]),
            writes: ComponentSet::default(),
            withs: F::withs(registry),
            withouts: F::withouts(registry),
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

    fn fetch(
        entity: Entity,
        data: &'a ComponentMap<&'a LockedData>,
        registry: &'a Registry,
    ) -> Option<Self::ItemRef> {
        let data = data.get(&registry.get_static::<T>())?;
        let data = data.borrow_mut();

        Some(Mut {
            entity,
            component: AtomicRefMut::map(data, |data| data.get_as_mut::<T>().unwrap()),
        })
    }

    fn access(registry: &Registry) -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::from_iter([registry.get_static::<T>()]),
            withs: F::withs(registry),
            withouts: F::withouts(registry),
        }
    }
}

impl<'a, F> Queryable<'a, F> for Entity
where
    F: QueryFilter<'a>,
{
    type Item = Entity;
    type ItemRef = Entity;

    fn fetch(
        entity: Entity,
        _data: &'a ComponentMap<&'a LockedData>,
        _registry: &'a Registry,
    ) -> Option<Self::ItemRef> {
        Some(entity)
    }

    fn access(registry: &Registry) -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::default(),
            withs: F::withs(registry),
            withouts: F::withouts(registry),
        }
    }
}

/// Very similar to a Queryable, but instead of yielding a reference to the component, it is just used for filtering.
pub trait QueryFilter<'a> {
    fn withs(_registry: &Registry) -> ComponentSet {
        ComponentSet::default()
    }
    fn withouts(_registry: &Registry) -> ComponentSet {
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
    fn withs(registry: &Registry) -> ComponentSet {
        ComponentSet::from_iter([registry.get_static::<T>()])
    }
}

pub struct Without<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for Without<'a, T>
where
    T: Component,
{
    fn withouts(registry: &Registry) -> ComponentSet {
        ComponentSet::from_iter([registry.get_static::<T>()])
    }
}

pub struct Query<'a, Q, F = ()>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    registry: &'a Registry,
    entries: SparseSet<Entity, ComponentMap<&'a LockedData>>,
    _marker: std::marker::PhantomData<(Q, F)>,
}

impl<'a, Q, F> Query<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub(crate) fn new(components: &'a Components) -> Self {
        let registry = components.registry();
        let entries = components.components_matching_access(Q::access(registry));
        Query {
            registry,
            entries,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn get(&'a self, entity: Entity) -> Option<Q::ItemRef> {
        let data = self.entries.get(&entity)?;
        Q::fetch(entity, data, self.registry)
    }

    pub fn iter(&'a self) -> impl Iterator<Item = Q::ItemRef> + 'a {
        self.entries
            .iter()
            .filter_map(|(entity, data)| Q::fetch(*entity, data, self.registry))
    }

    pub fn access(&self) -> QueryAccess {
        Q::access(self.registry)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DynamicQueryParam {
    Read(DynamicId),
    Write(DynamicId),
    With(DynamicId),
    Without(DynamicId),
}

#[derive(Clone, Debug, Default)]
pub struct DynamicQueryParams {
    params: Vec<DynamicQueryParam>,
}

impl DynamicQueryParams {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn read(mut self, id: DynamicId) -> Self {
        self.params.push(DynamicQueryParam::Read(id));
        self
    }

    #[must_use]
    pub fn write(mut self, id: DynamicId) -> Self {
        self.params.push(DynamicQueryParam::Write(id));
        self
    }

    #[must_use]
    pub fn with(mut self, id: DynamicId) -> Self {
        self.params.push(DynamicQueryParam::With(id));
        self
    }

    #[must_use]
    pub fn without(mut self, id: DynamicId) -> Self {
        self.params.push(DynamicQueryParam::Without(id));
        self
    }
}

impl Deref for DynamicQueryParams {
    type Target = [DynamicQueryParam];

    fn deref(&self) -> &Self::Target {
        &self.params
    }
}

pub enum DynamicQueryRef<'a> {
    Ref(AtomicRef<'a, Data>),
    Mut(AtomicRefMut<'a, Data>),
}

pub struct DynamicQuery<'a> {
    entries: SparseSet<Entity, ComponentMap<&'a LockedData>>,
    params: DynamicQueryParams,
    access: QueryAccess,
}

impl<'a> DynamicQuery<'a> {
    pub fn builder(components: &'a Components) -> DynamicQueryBuilder {
        DynamicQueryBuilder::new(components)
    }

    pub(crate) fn new(components: &'a Components, params: DynamicQueryParams) -> Self {
        let mut access = QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::default(),
            withs: ComponentSet::default(),
            withouts: ComponentSet::default(),
        };

        for param in params.iter().copied() {
            match param {
                DynamicQueryParam::Read(id) => {
                    access.reads.insert(id, ());
                }
                DynamicQueryParam::Write(id) => {
                    access.writes.insert(id, ());
                }
                DynamicQueryParam::With(id) => {
                    access.withs.insert(id, ());
                }
                DynamicQueryParam::Without(id) => {
                    access.withouts.insert(id, ());
                }
            }
        }

        let entries = components.components_matching_access(access);

        Self {
            entries,
            params,
            access: QueryAccess::default(),
        }
    }

    pub fn get(&'a self, entity: &'a Entity) -> Option<Vec<DynamicQueryRef<'a>>> {
        let data = self.entries.get(entity)?;
        let mut refs = Vec::new();
        for param in self.params.iter() {
            match param {
                DynamicQueryParam::Read(id) => {
                    let data = data.get(id)?;
                    refs.push(DynamicQueryRef::Ref(data.borrow()));
                }
                DynamicQueryParam::Write(id) => {
                    let data = data.get(id)?;
                    refs.push(DynamicQueryRef::Mut(data.borrow_mut()));
                }
                DynamicQueryParam::With(_) => {}
                DynamicQueryParam::Without(_) => {}
            }
        }
        Some(refs)
    }

    pub fn iter(&'a self) -> impl Iterator<Item = Vec<DynamicQueryRef<'a>>> + 'a {
        self.entries
            .iter()
            .filter_map(|(entity, _)| self.get(entity))
    }

    pub fn access(&self) -> &QueryAccess {
        &self.access
    }
}

pub struct DynamicQueryBuilder<'a> {
    components: &'a Components,
    params: DynamicQueryParams,
}

impl<'a> DynamicQueryBuilder<'a> {
    pub fn new(components: &'a Components) -> Self {
        Self {
            components,
            params: DynamicQueryParams::new(),
        }
    }

    #[must_use]
    pub fn read<T: Component>(mut self) -> Self {
        self.params = self
            .params
            .read(self.components.registry().get_static::<T>());
        self
    }

    #[must_use]
    pub fn write<T: Component>(mut self) -> Self {
        self.params = self
            .params
            .write(self.components.registry().get_static::<T>());
        self
    }

    #[must_use]
    pub fn with<T: Component>(mut self) -> Self {
        self.params = self
            .params
            .with(self.components.registry().get_static::<T>());
        self
    }

    #[must_use]
    pub fn without<T: Component>(mut self) -> Self {
        self.params = self
            .params
            .without(self.components.registry().get_static::<T>());
        self
    }

    #[must_use]
    pub fn read_id(mut self, id: DynamicId) -> Self {
        self.params = self.params.read(id);
        self
    }

    #[must_use]
    pub fn write_id(mut self, id: DynamicId) -> Self {
        self.params = self.params.write(id);
        self
    }

    #[must_use]
    pub fn with_id(mut self, id: DynamicId) -> Self {
        self.params = self.params.with(id);
        self
    }

    #[must_use]
    pub fn without_id(mut self, id: DynamicId) -> Self {
        self.params = self.params.without(id);
        self
    }

    #[must_use]
    pub fn build(self) -> DynamicQuery<'a> {
        DynamicQuery::new(self.components, self.params)
    }
}

macro_rules! impl_queryable_for_tuple {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<'a, $($name),*, Filter> Queryable<'a, Filter> for ($($name,)*)
        where
            $($name: Queryable<'a, Filter>,)*
            Filter: QueryFilter<'a>,
            ($($name::Item,)*) : Bundle,
        {
            type Item = ($($name::Item,)*);
            type ItemRef = ($($name::ItemRef,)*);

            fn fetch(entity: Entity, data: &'a ComponentMap<&'a LockedData>, registry: &'a Registry) -> Option<Self::ItemRef> {
                Some(($($name::fetch(entity, data, registry)?,)*))
            }

            fn access(registry: &Registry) -> QueryAccess {
                let mut reads = ComponentSet::default();
                let mut writes = ComponentSet::default();
                let mut withs = ComponentSet::default();
                let mut withouts = ComponentSet::default();

                $({
                    let access = $name::access(registry);
                    reads.extend(access.reads);
                    writes.extend(access.writes);
                    withs.extend(access.withs);
                    withouts.extend(access.withouts);
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

all_tuples!(1..=16, impl_queryable_for_tuple);

macro_rules! impl_queryfilter_for_tuple {
    ($($name:ident),*) => {
        impl<'a, $($name),*> QueryFilter<'a> for ($($name,)*)
        where
            $($name: QueryFilter<'a>,)*
        {
            fn withs(registry: &Registry) -> ComponentSet {
                let mut all = ComponentSet::default();
                $(
                    all.extend($name::withs(registry));
                )*
                all
            }

            fn withouts(registry: &Registry) -> ComponentSet {
                let mut all = ComponentSet::default();
                $(
                    all.extend($name::withouts(registry));
                )*
                all
            }
        }
    };
}

all_tuples!(1..=16, impl_queryfilter_for_tuple);
