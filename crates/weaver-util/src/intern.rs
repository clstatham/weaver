//! Interning: storing one leaked copy of a value so it can be compared and hashed by address.
//!
//! Mostly taken from `bevy_ecs`. Used for labels (see [`define_label!`](crate::define_label)),
//! where the same label gets created in many places and has to compare cheaply.

use std::{
    hash::{Hash, Hasher},
    ops::Deref,
    sync::OnceLock,
};

use hashbrown::HashSet;

use crate::lock::Lock;

/// A copyable handle to an interned value.
///
/// Equality and hashing look at the address (and length, for slices), not the contents, so two
/// handles are equal only if they came from the same [`Interner`].
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

/// Types that [`Interner`] can store.
pub trait Internable: Hash + Eq {
    /// Copies `self` into a `'static` allocation that is never freed.
    fn leak(&self) -> &'static Self;

    /// Compares by address, as opposed to [`Eq`] which compares contents.
    fn ref_eq(&self, other: &Self) -> bool;

    /// Hashes the address, consistent with [`Internable::ref_eq`].
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

/// A global table of interned values of type `T`.
///
/// Meant to live in a `static` (hence the `const` constructor). Interned values are leaked and
/// never removed.
pub struct Interner<T: Internable + ?Sized + 'static>(OnceLock<Lock<HashSet<&'static T>>>);

impl<T: Internable + ?Sized> Interner<T> {
    /// Creates an empty interner.
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    /// Returns the interned copy of `value`, storing one first if this is the first time it's seen.
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
