use std::{any::TypeId, marker::PhantomData};

use weaver_util::prelude::*;

use crate::{
    change_detection::{ComponentTicks, Tick},
    entity::Entity,
    prelude::{Archetype, Component, Res, SystemAccess, SystemParam},
    storage::{Access, Components, Mut, Ref},
};

use super::world::World;

/// The change ticks a query compares component ticks against.
#[derive(Debug, Clone, Copy)]
pub struct TickRange {
    pub last_run: Tick,
    pub this_run: Tick,
}

/// One component type a query touches, and how.
#[derive(Debug, Clone, Copy)]
pub struct AccessTerm {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub access: Access,
}

/// The component types a [`Queryable`] reads and writes, deduplicated.
#[derive(Debug, Default)]
pub struct QueryAccess {
    terms: Vec<AccessTerm>,
}

impl QueryAccess {
    /// Collects `Q`'s access. Panics if `Q` would alias a component, e.g. `(&mut A, &A)`.
    pub fn of<Q: Queryable>() -> Self {
        let mut access = Self::default();
        Q::access(&mut access);
        access
    }

    pub fn read<T: Component>(&mut self) {
        self.add::<T>(Access::Read);
    }

    pub fn write<T: Component>(&mut self) {
        self.add::<T>(Access::Write);
    }

    fn add<T: Component>(&mut self, access: Access) {
        let type_id = TypeId::of::<T>();
        if let Some(existing) = self.terms.iter().find(|term| term.type_id == type_id) {
            if existing.access == Access::Read && access == Access::Read {
                return;
            }
            panic!(
                "a query accesses {} twice ({:?} and {:?}); a write can't overlap any other access to the same component",
                existing.type_name, existing.access, access
            );
        }
        self.terms.push(AccessTerm {
            type_id,
            type_name: std::any::type_name::<T>(),
            access,
        });
    }

    pub fn terms(&self) -> &[AccessTerm] {
        &self.terms
    }
}

/// A term of a [`Query`]: a component reference, an [`Entity`], a filter, or a tuple of these.
///
/// A query matches whole archetypes with [`Queryable::matches`], then visits each row of every
/// matching archetype, where [`Queryable::fetch_row`] can still filter out single rows.
///
/// # Safety
///
/// An implementation must only access component data it declared in [`Queryable::access`],
/// and only with the declared kind of access.
pub unsafe trait Queryable: Send + Sync {
    type Item<'w>: Send + Sync;
    /// Per-archetype state, such as column pointers that were downcast once.
    type Fetch<'w>;

    fn access(access: &mut QueryAccess);

    /// Returns true if rows of this archetype can be yielded.
    fn matches(archetype: &Archetype) -> bool;

    /// # Safety
    ///
    /// `matches(archetype)` must be true, and the borrows in `access()` must be held for `'w`.
    unsafe fn fetch_archetype<'w>(archetype: &'w Archetype, ticks: TickRange) -> Self::Fetch<'w>;

    /// Returns the item at `row`, or `None` if that row is filtered out.
    ///
    /// # Safety
    ///
    /// `row` must be in bounds for the archetype `fetch` was made from, and no other item for
    /// the same row and fetch may be alive.
    unsafe fn fetch_row<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Option<Self::Item<'w>>;
}

pub type QueryableItem<'a, T> = <T as Queryable>::Item<'a>;

unsafe impl Queryable for Entity {
    type Item<'w> = Entity;
    type Fetch<'w> = &'w [Entity];

    fn access(_access: &mut QueryAccess) {}

    fn matches(_archetype: &Archetype) -> bool {
        true
    }

    unsafe fn fetch_archetype<'w>(archetype: &'w Archetype, _ticks: TickRange) -> Self::Fetch<'w> {
        archetype.entities()
    }

    unsafe fn fetch_row<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Option<Self::Item<'w>> {
        debug_assert!(row < fetch.len());
        // SAFETY: the caller guarantees `row` is in bounds
        Some(unsafe { *fetch.get_unchecked(row) })
    }
}

#[doc(hidden)]
pub struct ReadFetch<'w, T> {
    data: &'w [T],
    ticks: &'w [ComponentTicks],
    range: TickRange,
}

impl<'w, T: Component> ReadFetch<'w, T> {
    /// # Safety
    ///
    /// The archetype must contain `T`, and a read borrow of `T` must be held for `'w`.
    unsafe fn new(archetype: &'w Archetype, range: TickRange) -> Self {
        let column = archetype.column_of::<T>().unwrap();
        // SAFETY: the caller holds a read borrow of T
        unsafe {
            Self {
                data: column.data_slice::<T>(),
                ticks: column.ticks_slice(),
                range,
            }
        }
    }

    /// # Safety
    ///
    /// `row` must be in bounds.
    unsafe fn ticks(&self, row: usize) -> &'w ComponentTicks {
        debug_assert!(row < self.ticks.len());
        // SAFETY: the caller guarantees `row` is in bounds
        unsafe { self.ticks.get_unchecked(row) }
    }

    /// # Safety
    ///
    /// `row` must be in bounds.
    unsafe fn get(&self, row: usize) -> Ref<'w, T> {
        debug_assert!(row < self.data.len());
        // SAFETY: the caller guarantees `row` is in bounds
        unsafe {
            Ref::new(
                self.data.get_unchecked(row),
                self.range.last_run,
                self.range.this_run,
                self.ticks(row),
            )
        }
    }
}

#[doc(hidden)]
pub struct WriteFetch<'w, T> {
    data: *mut T,
    ticks: *mut ComponentTicks,
    #[cfg(debug_assertions)]
    len: usize,
    range: TickRange,
    _marker: PhantomData<&'w mut T>,
}

impl<'w, T: Component> WriteFetch<'w, T> {
    /// # Safety
    ///
    /// The archetype must contain `T`, and the write borrow of `T` must be held for `'w`.
    unsafe fn new(archetype: &'w Archetype, range: TickRange) -> Self {
        let column = archetype.column_of::<T>().unwrap();
        // SAFETY: the caller holds the write borrow of T
        unsafe {
            Self {
                data: column.data_ptr::<T>(),
                ticks: column.ticks_ptr(),
                #[cfg(debug_assertions)]
                len: archetype.len(),
                range,
                _marker: PhantomData,
            }
        }
    }

    /// # Safety
    ///
    /// `row` must be in bounds.
    unsafe fn ticks(&self, row: usize) -> &'w ComponentTicks {
        #[cfg(debug_assertions)]
        debug_assert!(row < self.len);
        // SAFETY: the caller guarantees `row` is in bounds
        unsafe { &*self.ticks.add(row) }
    }

    /// # Safety
    ///
    /// `row` must be in bounds, and no other reference to it may be alive.
    unsafe fn get(&self, row: usize) -> Mut<'w, T> {
        #[cfg(debug_assertions)]
        debug_assert!(row < self.len);
        // SAFETY: the caller guarantees `row` is in bounds and unaliased; the write borrow keeps
        // every other query out of this column
        unsafe {
            Mut::new(
                &mut *self.data.add(row),
                self.range.last_run,
                self.range.this_run,
                &mut *self.ticks.add(row),
            )
        }
    }
}

unsafe impl<T: Component> Queryable for &T {
    type Item<'w> = Ref<'w, T>;
    type Fetch<'w> = ReadFetch<'w, T>;

    fn access(access: &mut QueryAccess) {
        access.read::<T>();
    }

    fn matches(archetype: &Archetype) -> bool {
        archetype.has::<T>()
    }

    unsafe fn fetch_archetype<'w>(archetype: &'w Archetype, ticks: TickRange) -> Self::Fetch<'w> {
        // SAFETY: forwarded from the caller
        unsafe { ReadFetch::new(archetype, ticks) }
    }

    unsafe fn fetch_row<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Option<Self::Item<'w>> {
        // SAFETY: forwarded from the caller
        Some(unsafe { fetch.get(row) })
    }
}

unsafe impl<T: Component> Queryable for &mut T {
    type Item<'w> = Mut<'w, T>;
    type Fetch<'w> = WriteFetch<'w, T>;

    fn access(access: &mut QueryAccess) {
        access.write::<T>();
    }

    fn matches(archetype: &Archetype) -> bool {
        archetype.has::<T>()
    }

    unsafe fn fetch_archetype<'w>(archetype: &'w Archetype, ticks: TickRange) -> Self::Fetch<'w> {
        // SAFETY: forwarded from the caller
        unsafe { WriteFetch::new(archetype, ticks) }
    }

    unsafe fn fetch_row<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Option<Self::Item<'w>> {
        // SAFETY: forwarded from the caller
        Some(unsafe { fetch.get(row) })
    }
}

/// Matches every archetype, yielding `None` for rows of archetypes `Q` doesn't match.
unsafe impl<Q: Queryable> Queryable for Option<Q> {
    type Item<'w> = Option<Q::Item<'w>>;
    type Fetch<'w> = Option<Q::Fetch<'w>>;

    fn access(access: &mut QueryAccess) {
        Q::access(access);
    }

    fn matches(_archetype: &Archetype) -> bool {
        true
    }

    unsafe fn fetch_archetype<'w>(archetype: &'w Archetype, ticks: TickRange) -> Self::Fetch<'w> {
        // SAFETY: only called when `Q::matches` holds; borrows forwarded from the caller
        Q::matches(archetype).then(|| unsafe { Q::fetch_archetype(archetype, ticks) })
    }

    unsafe fn fetch_row<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Option<Self::Item<'w>> {
        // SAFETY: forwarded from the caller
        Some(
            fetch
                .as_mut()
                .and_then(|fetch| unsafe { Q::fetch_row(fetch, row) }),
        )
    }
}

/// Only matches archetypes that contain `T`. Doesn't access `T`'s data.
pub struct With<T: Component>(PhantomData<T>);

unsafe impl<T: Component> Queryable for With<T> {
    type Item<'w> = &'w ();
    type Fetch<'w> = ();

    fn access(_access: &mut QueryAccess) {}

    fn matches(archetype: &Archetype) -> bool {
        archetype.has::<T>()
    }

    unsafe fn fetch_archetype(_archetype: &Archetype, _ticks: TickRange) {}

    unsafe fn fetch_row<'w>(_fetch: &mut (), _row: usize) -> Option<Self::Item<'w>> {
        Some(&())
    }
}

/// Only matches archetypes that don't contain `T`.
pub struct Without<T: Component>(PhantomData<T>);

unsafe impl<T: Component> Queryable for Without<T> {
    type Item<'w> = &'w ();
    type Fetch<'w> = ();

    fn access(_access: &mut QueryAccess) {}

    fn matches(archetype: &Archetype) -> bool {
        !archetype.has::<T>()
    }

    unsafe fn fetch_archetype(_archetype: &Archetype, _ticks: TickRange) {}

    unsafe fn fetch_row<'w>(_fetch: &mut (), _row: usize) -> Option<Self::Item<'w>> {
        Some(&())
    }
}

/// Like `&T` or `&mut T`, but skips rows whose `T` hasn't changed since the system last ran.
pub struct Changed<T>(PhantomData<T>);

unsafe impl<T: Component> Queryable for Changed<&T> {
    type Item<'w> = Ref<'w, T>;
    type Fetch<'w> = ReadFetch<'w, T>;

    fn access(access: &mut QueryAccess) {
        access.read::<T>();
    }

    fn matches(archetype: &Archetype) -> bool {
        archetype.has::<T>()
    }

    unsafe fn fetch_archetype<'w>(archetype: &'w Archetype, ticks: TickRange) -> Self::Fetch<'w> {
        // SAFETY: forwarded from the caller
        unsafe { ReadFetch::new(archetype, ticks) }
    }

    unsafe fn fetch_row<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Option<Self::Item<'w>> {
        let range = fetch.range;
        // SAFETY: forwarded from the caller
        unsafe {
            fetch
                .ticks(row)
                .is_changed(range.last_run, range.this_run)
                .then(|| fetch.get(row))
        }
    }
}

unsafe impl<T: Component> Queryable for Changed<&mut T> {
    type Item<'w> = Mut<'w, T>;
    type Fetch<'w> = WriteFetch<'w, T>;

    fn access(access: &mut QueryAccess) {
        access.write::<T>();
    }

    fn matches(archetype: &Archetype) -> bool {
        archetype.has::<T>()
    }

    unsafe fn fetch_archetype<'w>(archetype: &'w Archetype, ticks: TickRange) -> Self::Fetch<'w> {
        // SAFETY: forwarded from the caller
        unsafe { WriteFetch::new(archetype, ticks) }
    }

    unsafe fn fetch_row<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Option<Self::Item<'w>> {
        let range = fetch.range;
        // SAFETY: forwarded from the caller; the ticks reference is dropped before `get` makes a `&mut` to them
        unsafe {
            fetch
                .ticks(row)
                .is_changed(range.last_run, range.this_run)
                .then(|| fetch.get(row))
        }
    }
}

macro_rules! impl_queryable_tuple {
    ($( $name:ident ),*) => {
        #[allow(non_snake_case)]
        unsafe impl<$($name: Queryable),*> Queryable for ($($name,)*) {
            type Item<'w> = ($($name::Item<'w>,)*);
            type Fetch<'w> = ($($name::Fetch<'w>,)*);

            fn access(access: &mut QueryAccess) {
                $($name::access(access);)*
            }

            fn matches(archetype: &Archetype) -> bool {
                $($name::matches(archetype))&&*
            }

            unsafe fn fetch_archetype<'w>(archetype: &'w Archetype, ticks: TickRange) -> Self::Fetch<'w> {
                // SAFETY: every element matches when the tuple does; borrows forwarded from the caller
                unsafe { ($($name::fetch_archetype(archetype, ticks),)*) }
            }

            unsafe fn fetch_row<'w>(fetch: &mut Self::Fetch<'w>, row: usize) -> Option<Self::Item<'w>> {
                let ($($name,)*) = fetch;
                // every element reads the same row, so a filtered element drops the whole row
                // SAFETY: forwarded from the caller
                unsafe { Some(($($name::fetch_row($name, row)?,)*)) }
            }
        }
    };
}

impl_queryable_tuple!(A);
impl_queryable_tuple!(A, B);
impl_queryable_tuple!(A, B, C);
impl_queryable_tuple!(A, B, C, D);
impl_queryable_tuple!(A, B, C, D, E);
impl_queryable_tuple!(A, B, C, D, E, F);
impl_queryable_tuple!(A, B, C, D, E, F, G);
impl_queryable_tuple!(A, B, C, D, E, F, G, H);

/// Iterates over or looks up entities whose components match `Q`.
///
/// A live query keeps a shared loan of the world's [`Components`] and borrows each component type
/// it touches. Another query that needs conflicting access to one of those types panics, and so
/// does any structural change (spawning, destroying, inserting or removing components) until
/// this query is dropped. Systems should use `Commands` for structural changes; those are applied
/// after the system's queries are gone.
pub struct Query<Q: Queryable> {
    components: Res<Components>,
    /// Indices of the non-empty archetypes that `Q` matches.
    matched: Vec<usize>,
    borrows: Vec<(TypeId, Access)>,
    ticks: TickRange,
    _marker: PhantomData<fn() -> Q>,
}

impl<Q: Queryable> Query<Q> {
    pub fn new(world: &World) -> Self {
        Self::new_with_ticks(
            world,
            TickRange {
                last_run: world.last_change_tick(),
                this_run: world.read_change_tick(),
            },
        )
    }

    pub(crate) fn new_with_ticks(world: &World, ticks: TickRange) -> Self {
        let access = QueryAccess::of::<Q>();
        let components = world.components();

        let mut borrows = Vec::with_capacity(access.terms().len());
        for term in access.terms() {
            if !components.try_borrow(term.type_id, term.access) {
                for &(ty, access) in &borrows {
                    components.release(ty, access);
                }
                panic!(
                    "Query<{}> needs {:?} access to {}, which conflicts with another live Query",
                    std::any::type_name::<Q>(),
                    term.access,
                    term.type_name,
                );
            }
            borrows.push((term.type_id, term.access));
        }

        // archetypes can't change while we hold the loan, so an empty one stays empty
        let matched = components
            .archetypes()
            .iter()
            .enumerate()
            .filter(|(_, archetype)| !archetype.is_empty() && Q::matches(archetype))
            .map(|(index, _)| index)
            .collect();

        Self {
            components,
            matched,
            borrows,
            ticks,
            _marker: PhantomData,
        }
    }

    pub fn iter(&mut self) -> QueryIter<'_, Q> {
        QueryIter {
            archetypes: self.components.archetypes(),
            matched: self.matched.iter(),
            fetch: None,
            row: 0,
            len: 0,
            ticks: self.ticks,
        }
    }

    pub fn get(&mut self, entity: Entity) -> Option<Q::Item<'_>> {
        let location = self.components.location(entity)?;
        let archetype = &self.components.archetypes()[location.archetype.as_usize()];
        if !Q::matches(archetype) {
            return None;
        }
        // SAFETY: the archetype matches, we hold Q's borrows, the row comes from the entity's
        // location, and `&mut self` means no other item from this query is alive
        unsafe {
            let mut fetch = Q::fetch_archetype(archetype, self.ticks);
            Q::fetch_row(&mut fetch, location.row)
        }
    }
}

impl<Q: Queryable> Drop for Query<Q> {
    fn drop(&mut self) {
        for &(ty, access) in &self.borrows {
            self.components.release(ty, access);
        }
    }
}

pub struct QueryIter<'w, Q: Queryable> {
    archetypes: &'w [Archetype],
    matched: std::slice::Iter<'w, usize>,
    fetch: Option<Q::Fetch<'w>>,
    row: usize,
    len: usize,
    ticks: TickRange,
}

impl<'w, Q: Queryable> Iterator for QueryIter<'w, Q> {
    type Item = Q::Item<'w>;

    // Forced: when this isn't inlined into the caller's loop, the iterator state stays in
    // memory and per-row iteration gets several times slower (see benches/ecs.rs `iter_write`).
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(fetch) = self.fetch.as_mut() {
                while self.row < self.len {
                    let row = self.row;
                    self.row += 1;
                    // SAFETY: `row < len` for the archetype this fetch was made from, and each
                    // row is visited once
                    if let Some(item) = unsafe { Q::fetch_row(fetch, row) } {
                        return Some(item);
                    }
                }
            }

            let archetype = &self.archetypes[*self.matched.next()?];
            // SAFETY: `matched` only holds archetypes that `Q` matches, and the query that made
            // this iterator holds Q's borrows for `'w`
            self.fetch = Some(unsafe { Q::fetch_archetype(archetype, self.ticks) });
            self.row = 0;
            self.len = archetype.len();
        }
    }

    /// Internal iteration (`sum`, `count`, `for_each`, ...) gets a tight loop per archetype,
    /// which optimizes much better than repeated `next` calls.
    #[inline]
    fn fold<B, F>(mut self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let mut acc = init;
        if let Some(fetch) = self.fetch.as_mut() {
            for row in self.row..self.len {
                // SAFETY: same as in `next`
                if let Some(item) = unsafe { Q::fetch_row(fetch, row) } {
                    acc = f(acc, item);
                }
            }
        }
        for &index in self.matched {
            let archetype = &self.archetypes[index];
            // SAFETY: same as in `next`
            let mut fetch = unsafe { Q::fetch_archetype(archetype, self.ticks) };
            for row in 0..archetype.len() {
                // SAFETY: same as in `next`
                if let Some(item) = unsafe { Q::fetch_row(&mut fetch, row) } {
                    acc = f(acc, item);
                }
            }
        }
        acc
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.row
            + self
                .matched
                .clone()
                .map(|&index| self.archetypes[index].len())
                .sum::<usize>();
        (0, Some(remaining))
    }
}

impl<Q: Queryable> SystemParam for Query<Q> {
    type Item = Query<Q>;
    type State = ();

    fn access() -> SystemAccess {
        let mut access = SystemAccess {
            resources_read: FxHashSet::from_iter([TypeId::of::<Components>()]),
            ..SystemAccess::default()
        };
        for term in QueryAccess::of::<Q>().terms() {
            match term.access {
                Access::Read => access.components_read.insert(term.type_id),
                Access::Write => access.components_written.insert(term.type_id),
            };
        }
        access
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _state: &Self::State) -> Self::Item {
        Query::new(world)
    }
}
