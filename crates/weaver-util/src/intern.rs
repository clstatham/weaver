//! This is mostly taken from `bevy-ecs`

use std::{
    hash::{Hash, Hasher},
    ops::Deref,
    sync::OnceLock,
};

use hashbrown::HashSet;

use crate::lock::Lock;

#[derive(Debug)]
pub struct Interned<T: ?Sized + 'static>(&'static T);

impl<T: ?Sized> Clone for Interned<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Interned<T> {}

impl<T: ?Sized + 'static> Deref for Interned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T: Internable + ?Sized> PartialEq for Interned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.ref_eq(other.0)
    }
}

impl<T: Internable + ?Sized> Eq for Interned<T> {}

impl<T: Internable + ?Sized> Hash for Interned<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.ref_hash(state)
    }
}

pub trait Internable: Hash + Eq {
    fn leak(&self) -> &'static Self;

    fn ref_eq(&self, other: &Self) -> bool;

    fn ref_hash<H: Hasher>(&self, state: &mut H);
}

impl Internable for str {
    fn leak(&self) -> &'static Self {
        let boxed = self.to_string().into_boxed_str();
        Box::leak(boxed)
    }

    fn ref_eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr() && self.len() == other.len()
    }

    fn ref_hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        self.as_ptr().hash(state);
    }
}

pub struct Interner<T: Internable + ?Sized + 'static>(OnceLock<Lock<HashSet<&'static T>>>);

impl<T: Internable + ?Sized> Interner<T> {
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    pub fn intern(&self, value: &T) -> Interned<T> {
        let lock = self.0.get_or_init(|| Lock::new(HashSet::new()));
        {
            let set = lock.read();
            if let Some(interned) = set.get(value) {
                return Interned(*interned);
            }
        }
        {
            let mut set = lock.write();
            if let Some(interned) = set.get(value) {
                Interned(*interned)
            } else {
                let leaked = value.leak();
                set.insert(leaked);
                Interned(leaked)
            }
        }
    }
}

impl<T: Internable + ?Sized> Default for Interner<T> {
    fn default() -> Self {
        Self::new()
    }
}
