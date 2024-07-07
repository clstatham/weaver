use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Weak},
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};

#[derive(Default)]
#[repr(transparent)]
pub struct BorrowLock(AtomicRefCell<()>);

impl BorrowLock {
    pub const fn new() -> Self {
        Self(AtomicRefCell::new(()))
    }

    #[inline]
    pub fn borrow(&self) -> Borrow<'_> {
        Borrow(self.0.borrow())
    }

    #[inline]
    pub fn can_borrow(&self) -> bool {
        self.0.try_borrow().is_ok()
    }

    #[inline]
    pub fn try_borrow(&self) -> Option<Borrow<'_>> {
        self.0.try_borrow().ok().map(Borrow)
    }

    #[inline]
    pub fn can_borrow_mut(&self) -> bool {
        self.0.try_borrow_mut().is_ok()
    }

    #[inline]
    pub fn borrow_mut(&self) -> BorrowMut<'_> {
        BorrowMut(self.0.borrow_mut())
    }

    #[inline]
    pub fn try_borrow_mut(&self) -> Option<BorrowMut<'_>> {
        self.0.try_borrow_mut().ok().map(BorrowMut)
    }
}

#[repr(transparent)]
pub struct Borrow<'a>(AtomicRef<'a, ()>);

#[repr(transparent)]
pub struct BorrowMut<'a>(AtomicRefMut<'a, ()>);

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
