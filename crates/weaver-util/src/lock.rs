//! Lock types used for runtime borrow tracking.
//!
//! [`Lock`] and [`SharedLock`] wrap a `parking_lot::RwLock`. The difference from using the raw
//! lock is in the guards: [`Read`]/[`Write`] borrow the lock and can be mapped to a field
//! ([`MapRead`], [`MapWrite`]), while [`OwnedRead`]/[`OwnedWrite`] hold an `Arc` to the lock and so
//! aren't tied to a borrow, which lets the ECS keep a loan on a resource or component column
//! alive for as long as a system's parameters are. [`SyncCell`] is unrelated to locking but lives
//! here because it solves a similar problem (storing `!Sync` data in `Sync` containers).

use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Weak},
};

use parking_lot::{
    ArcRwLockReadGuard, ArcRwLockWriteGuard, MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock,
    RwLockReadGuard, RwLockWriteGuard,
};

/// A reader-writer lock that hands out [`Read`] and [`Write`] guards.
///
/// Plain wrapper over `parking_lot::RwLock`; use [`SharedLock`] when the guard needs to own the
/// lock.
#[derive(Debug, Default)]
pub struct Lock<T>(RwLock<T>);

impl<T> Lock<T> {
    /// Creates a lock holding `value`.
    pub fn new(value: T) -> Self {
        Self(RwLock::new(value))
    }

    /// Blocks until a shared read guard can be taken.
    pub fn read(&self) -> Read<'_, T> {
        Read::new(self)
    }

    /// Takes a read guard if the lock isn't write-locked, otherwise returns `None`.
    pub fn try_read(&self) -> Option<Read<'_, T>> {
        Read::try_new(self)
    }

    /// Blocks until the exclusive write guard can be taken.
    pub fn write(&self) -> Write<'_, T> {
        Write::new(self)
    }

    /// Takes the write guard if the lock is free, otherwise returns `None`.
    pub fn try_write(&self) -> Option<Write<'_, T>> {
        Write::try_new(self)
    }

    /// Consumes the lock and returns the value inside.
    pub fn into_inner(self) -> T {
        RwLock::into_inner(self.0)
    }
}

impl<T: Clone> From<T> for Lock<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

/// Shared guard for a [`Lock`].
#[derive(Debug)]
pub struct Read<'a, T>(RwLockReadGuard<'a, T>);
/// Exclusive guard for a [`Lock`].
#[derive(Debug)]
pub struct Write<'a, T>(RwLockWriteGuard<'a, T>);

impl<'a, T> Read<'a, T> {
    /// Blocks until a read guard on `lock` can be taken.
    pub fn new(lock: &'a Lock<T>) -> Self {
        Self(lock.0.read())
    }

    /// Takes a read guard on `lock` if it isn't write-locked.
    pub fn try_new(lock: &'a Lock<T>) -> Option<Self> {
        lock.0.try_read().map(Self)
    }

    /// Unwraps the underlying `parking_lot` guard.
    pub fn into_inner(self) -> RwLockReadGuard<'a, T> {
        self.0
    }

    /// Narrows the guard to a part of the locked value, keeping the lock held.
    pub fn map<U>(this: Self, f: impl FnOnce(&T) -> &U) -> MapRead<'a, U> {
        MapRead(RwLockReadGuard::map(this.0, f))
    }
}

impl<'a, T> Write<'a, T> {
    /// Blocks until the write guard on `lock` can be taken.
    pub fn new(lock: &'a Lock<T>) -> Self {
        Self(lock.0.write())
    }

    /// Takes the write guard on `lock` if it's free.
    pub fn try_new(lock: &'a Lock<T>) -> Option<Self> {
        lock.0.try_write().map(Self)
    }

    /// Unwraps the underlying `parking_lot` guard.
    pub fn into_inner(self) -> RwLockWriteGuard<'a, T> {
        self.0
    }

    /// Narrows the guard to a part of the locked value, keeping the lock held.
    pub fn map<U>(this: Self, f: impl FnOnce(&mut T) -> &mut U) -> MapWrite<'a, U> {
        MapWrite(RwLockWriteGuard::map(this.0, f))
    }
}

impl<T> Deref for Read<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Deref for Write<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Write<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A [`Read`] guard narrowed to part of the locked value (see [`Read::map`]).
pub struct MapRead<'a, T: ?Sized>(MappedRwLockReadGuard<'a, T>);

impl<T: ?Sized> Deref for MapRead<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized> MapRead<'a, T> {
    /// Unwraps the underlying `parking_lot` guard.
    pub fn into_inner(self) -> MappedRwLockReadGuard<'a, T> {
        self.0
    }

    /// Narrows the guard further.
    pub fn map<U>(this: Self, f: impl FnOnce(&T) -> &U) -> MapRead<'a, U> {
        MapRead(MappedRwLockReadGuard::map(this.0, f))
    }
}

/// A [`Write`] guard narrowed to part of the locked value (see [`Write::map`]).
pub struct MapWrite<'a, T: ?Sized>(MappedRwLockWriteGuard<'a, T>);

impl<T: ?Sized> Deref for MapWrite<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for MapWrite<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, T: ?Sized> MapWrite<'a, T> {
    /// Unwraps the underlying `parking_lot` guard.
    pub fn into_inner(self) -> MappedRwLockWriteGuard<'a, T> {
        self.0
    }

    /// Narrows the guard further.
    pub fn map<U>(this: Self, f: impl FnOnce(&mut T) -> &mut U) -> MapWrite<'a, U> {
        MapWrite(MappedRwLockWriteGuard::map(this.0, f))
    }
}

/// A reference-counted reader-writer lock whose guards own a handle to it.
///
/// Cloning gives another handle to the same lock. [`OwnedRead`] and [`OwnedWrite`] keep the lock
/// alive themselves, so they can be stored in structs without a lifetime. The ECS relies on this
/// to hold loans on resources and component columns for the length of a system run.
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct SharedLock<T: ?Sized>(Arc<RwLock<T>>);

impl<T> SharedLock<T> {
    /// Creates a lock holding `value`.
    pub fn new(value: T) -> Self {
        Self(Arc::new(RwLock::new(value)))
    }

    /// Returns a weak handle to the underlying lock.
    pub fn downgrade(&self) -> Weak<RwLock<T>> {
        Arc::downgrade(&self.0)
    }

    /// Blocks until an owned read guard can be taken.
    pub fn read(&self) -> OwnedRead<T> {
        OwnedRead::new(self)
    }

    /// Blocks until the owned write guard can be taken.
    pub fn write(&self) -> OwnedWrite<T> {
        OwnedWrite::new(self)
    }

    /// Returns `None` instead of blocking if the lock is write-locked.
    pub fn try_read(&self) -> Option<OwnedRead<T>> {
        self.0.clone().try_read_arc().map(OwnedRead)
    }

    /// Returns `None` instead of blocking if the lock is locked.
    pub fn try_write(&self) -> Option<OwnedWrite<T>> {
        self.0.clone().try_write_arc().map(OwnedWrite)
    }

    /// Number of handles (including live guards) pointing at this lock.
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }

    /// Returns the value if this is the only handle, otherwise `None`.
    pub fn into_inner(self) -> Option<T> {
        Some(RwLock::into_inner(Arc::into_inner(self.0)?))
    }
}

impl<T: ?Sized> Clone for SharedLock<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> From<T> for SharedLock<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: ?Sized> Deref for SharedLock<T> {
    type Target = RwLock<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Shared guard for a [`SharedLock`] that owns a handle to the lock.
#[derive(Debug)]
#[repr(transparent)]
pub struct OwnedRead<T: ?Sized>(ArcRwLockReadGuard<parking_lot::RawRwLock, T>);

impl<T: ?Sized> OwnedRead<T> {
    /// Blocks until a read guard on `lock` can be taken.
    pub fn new(lock: &SharedLock<T>) -> Self {
        Self(lock.0.clone().read_arc())
    }
}

impl<T: ?Sized> Deref for OwnedRead<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Exclusive guard for a [`SharedLock`] that owns a handle to the lock.
#[derive(Debug)]
#[repr(transparent)]
pub struct OwnedWrite<T: ?Sized>(ArcRwLockWriteGuard<parking_lot::RawRwLock, T>);

impl<T: ?Sized> OwnedWrite<T> {
    /// Blocks until the write guard on `lock` can be taken.
    pub fn new(lock: &SharedLock<T>) -> Self {
        Self(lock.0.clone().write_arc())
    }
}

impl<T: ?Sized> Deref for OwnedWrite<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for OwnedWrite<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Makes any `T` usable in a `Sync` container by only allowing access through `&mut`.
///
/// Since `&SyncCell<T>` exposes nothing (except `&T` when `T: Sync` anyway), sharing it between
/// threads can't cause a data race. Same idea as the unstable `std::sync::Exclusive`.
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct SyncCell<T: ?Sized>(T);

impl<T: Sized> SyncCell<T> {
    /// Wraps `value`.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Unwraps the cell.
    pub fn into_inner(Self(value): Self) -> T {
        value
    }
}

impl<T: ?Sized> SyncCell<T> {
    /// Shared access to the value, only available when `T` is itself `Sync`.
    pub fn read(&self) -> &T
    where
        T: Sync,
    {
        &self.0
    }

    /// Exclusive access to the value.
    pub fn get(&mut self) -> &mut T {
        &mut self.0
    }

    /// Reinterprets a `&mut T` as a `&mut SyncCell<T>`.
    pub fn from_mut(value: &mut T) -> &mut Self {
        // SAFETY: `SyncCell<T>` is `repr(transparent)` over `T`, so the layouts match, and the
        // exclusive borrow carries over to the returned reference.
        unsafe { &mut *(std::ptr::from_mut(value) as *mut SyncCell<T>) }
    }
}

// SAFETY: a shared `&SyncCell<T>` only gives out `&T` through `read`, which requires `T: Sync`.
// Everything else needs `&mut self`.
unsafe impl<T: ?Sized> Sync for SyncCell<T> {}
