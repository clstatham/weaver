use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Weak},
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};

#[derive(Debug, Default)]
pub struct Lock<T>(AtomicRefCell<T>);

impl<T> Lock<T> {
    pub fn new(value: T) -> Self {
        Self(AtomicRefCell::new(value))
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
}

impl<T: Clone> From<T> for Lock<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[derive(Debug)]
pub struct Read<'a, T>(AtomicRef<'a, T>);
#[derive(Debug)]
pub struct Write<'a, T>(AtomicRefMut<'a, T>);

impl<'a, T> Read<'a, T> {
    pub fn new(lock: &'a Lock<T>) -> Self {
        Self(lock.0.borrow())
    }

    pub fn try_new(lock: &'a Lock<T>) -> Option<Self> {
        lock.0.try_borrow().ok().map(Self)
    }

    pub fn into_inner(self) -> AtomicRef<'a, T> {
        self.0
    }

    pub fn map_read<U, F>(self, f: F) -> Read<'a, U>
    where
        F: FnOnce(&T) -> &U,
    {
        Read(AtomicRef::map(self.0, f))
    }
}

impl<'a, T> Write<'a, T> {
    pub fn new(lock: &'a Lock<T>) -> Self {
        Self(lock.0.borrow_mut())
    }

    pub fn try_new(lock: &'a Lock<T>) -> Option<Self> {
        lock.0.try_borrow_mut().ok().map(Self)
    }

    pub fn into_inner(self) -> AtomicRefMut<'a, T> {
        self.0
    }

    pub fn map_write<U, F>(self, f: F) -> Write<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        Write(AtomicRefMut::map(self.0, f))
    }
}

impl<'a, T> Deref for Read<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T> Deref for Write<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T> DerefMut for Write<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct SharedLock<T: ?Sized>(Arc<AtomicRefCell<T>>);

impl<T> SharedLock<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(AtomicRefCell::new(value)))
    }

    pub fn downgrade(&self) -> Weak<AtomicRefCell<T>> {
        Arc::downgrade(&self.0)
    }

    pub fn read(&self) -> Read<'_, T> {
        Read(self.0.borrow())
    }

    pub fn write(&self) -> Write<'_, T> {
        Write(self.0.borrow_mut())
    }

    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }

    pub fn into_inner(self) -> Option<T> {
        Some(AtomicRefCell::into_inner(Arc::into_inner(self.0)?))
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
    type Target = AtomicRefCell<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
