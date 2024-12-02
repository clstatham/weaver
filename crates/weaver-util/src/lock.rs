use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Weak},
};

use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

#[derive(Debug, Default)]
pub struct Lock<T>(RwLock<T>);

impl<T> Lock<T> {
    pub fn new(value: T) -> Self {
        Self(RwLock::new(value))
    }

    pub fn read(&self) -> Read<'_, T> {
        Read::new(self)
    }

    pub fn try_read(&self) -> Option<Read<'_, T>> {
        Read::try_new(self)
    }

    pub fn write(&self) -> Write<'_, T> {
        Write::new(self)
    }

    pub fn try_write(&self) -> Option<Write<'_, T>> {
        Write::try_new(self)
    }

    pub fn into_inner(self) -> T {
        RwLock::into_inner(self.0)
    }
}

impl<T: Clone> From<T> for Lock<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[derive(Debug)]
pub struct Read<'a, T>(RwLockReadGuard<'a, T>);
#[derive(Debug)]
pub struct Write<'a, T>(RwLockWriteGuard<'a, T>);

impl<'a, T> Read<'a, T> {
    pub fn new(lock: &'a Lock<T>) -> Self {
        if cfg!(debug_assertions) && lock.0.try_read().is_none() {
            log::warn!("Read lock contention detected");
            let bt = std::backtrace::Backtrace::force_capture();
            log::warn!("{}", bt);
        }
        Self(lock.0.read())
    }

    pub fn try_new(lock: &'a Lock<T>) -> Option<Self> {
        lock.0.try_read().map(Self)
    }

    pub fn into_inner(self) -> RwLockReadGuard<'a, T> {
        self.0
    }

    pub fn map<U>(this: Self, f: impl FnOnce(&T) -> &U) -> MapRead<'a, U> {
        MapRead(RwLockReadGuard::map(this.0, f))
    }
}

impl<'a, T> Write<'a, T> {
    pub fn new(lock: &'a Lock<T>) -> Self {
        if cfg!(debug_assertions) && lock.0.try_write().is_none() {
            log::warn!("Write lock contention detected");
            let bt = std::backtrace::Backtrace::force_capture();
            log::warn!("{}", bt);
        }
        Self(lock.0.write())
    }

    pub fn try_new(lock: &'a Lock<T>) -> Option<Self> {
        lock.0.try_write().map(Self)
    }

    pub fn into_inner(self) -> RwLockWriteGuard<'a, T> {
        self.0
    }

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

pub struct MapRead<'a, T: ?Sized>(MappedRwLockReadGuard<'a, T>);

impl<T: ?Sized> Deref for MapRead<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized> MapRead<'a, T> {
    pub fn into_inner(self) -> MappedRwLockReadGuard<'a, T> {
        self.0
    }

    pub fn map<U>(this: Self, f: impl FnOnce(&T) -> &U) -> MapRead<'a, U> {
        MapRead(MappedRwLockReadGuard::map(this.0, f))
    }
}

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
    pub fn into_inner(self) -> MappedRwLockWriteGuard<'a, T> {
        self.0
    }

    pub fn map<U>(this: Self, f: impl FnOnce(&mut T) -> &mut U) -> MapWrite<'a, U> {
        MapWrite(MappedRwLockWriteGuard::map(this.0, f))
    }
}

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct SharedLock<T: ?Sized>(Arc<RwLock<T>>);

impl<T> SharedLock<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(RwLock::new(value)))
    }

    pub fn downgrade(&self) -> Weak<RwLock<T>> {
        Arc::downgrade(&self.0)
    }

    pub fn read(&self) -> Read<'_, T> {
        if cfg!(debug_assertions) && self.0.try_read().is_none() {
            log::warn!("Read lock contention detected");
            let bt = std::backtrace::Backtrace::force_capture();
            log::warn!("{}", bt);
        }
        Read(self.0.read())
    }

    pub fn write(&self) -> Write<'_, T> {
        if cfg!(debug_assertions) && self.0.try_write().is_none() {
            log::warn!("Write lock contention detected");
            let bt = std::backtrace::Backtrace::force_capture();
            log::warn!("{}", bt);
        }
        Write(self.0.write())
    }

    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }

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

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct SyncCell<T: ?Sized>(T);

impl<T: Sized> SyncCell<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn into_inner(Self(value): Self) -> T {
        value
    }
}

impl<T: ?Sized> SyncCell<T> {
    pub fn read(&self) -> &T
    where
        T: Sync,
    {
        &self.0
    }

    pub fn get(&mut self) -> &mut T {
        &mut self.0
    }

    pub fn from_mut(value: &mut T) -> &mut Self {
        unsafe { &mut *(std::ptr::from_mut(value) as *mut SyncCell<T>) }
    }
}

unsafe impl<T: ?Sized> Sync for SyncCell<T> {}
