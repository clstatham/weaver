//! Shared and exclusive loans of a value that stays owned by a [`LoanStorage`].
//!
//! Resources and singleton component data live in a [`LoanStorage`]. Borrowing one moves the value
//! out (exclusive, [`LoanMut`]) or shares it through an `Arc` ([`Loan`]), and the storage records
//! that it's out. Asking for a loan that conflicts with one already outstanding returns `None`
//! instead of blocking, so the caller decides whether that's a skip or a bug. Dropping a
//! [`LoanMut`] puts the value back.
//!
//! Much of the inspiration and implementation of this module is based on the `broomdog` crate, particularly the `Loan`, `LoanMut`, and `LoanStorage` types.
//! <https://github.com/schell/broomdog/>

use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use weaver_util::lock::SharedLock;

/// A shared, read-only loan of a value held by a [`LoanStorage`].
pub struct Loan<T>(Arc<T>);

impl<T> Loan<T> {
    /// Number of live handles to the loaned value, including the one kept by the storage.
    pub fn strong_count(this: &Self) -> usize {
        Arc::strong_count(&this.0)
    }
}

impl<T> Clone for Loan<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for Loan<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// An exclusive loan of a value held by a [`LoanStorage`].
///
/// The value is moved out of the storage for the lifetime of the loan and written back on drop.
pub struct LoanMut<T> {
    inner: Option<T>,
    outer: SharedLock<Option<T>>,
}

impl<T> Deref for LoanMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap()
    }
}

impl<T> DerefMut for LoanMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap()
    }
}

impl<T> Drop for LoanMut<T> {
    fn drop(&mut self) {
        *self.outer.write() = self.inner.take();
    }
}

/// A slot holding a value that can be lent out, shared or exclusively.
///
/// The slot is `Owned` when nothing is borrowed, `Loan` while shared loans exist, and `LoanMut`
/// while the value is moved into a [`LoanMut`]. `Vacant` is the empty state, and is also used
/// briefly while the state is being swapped.
#[derive(Default)]
pub enum LoanStorage<T> {
    /// Holds nothing.
    #[default]
    Vacant,
    /// Holds the value, with no loans out.
    Owned(T),
    /// The value is shared through one or more [`Loan`]s.
    Loan(Arc<T>),
    /// The value is moved into a [`LoanMut`]; this is the cell it returns to on drop.
    LoanMut(SharedLock<Option<T>>),
}

impl<T> LoanStorage<T> {
    /// Creates a storage that owns `value`.
    pub fn new(value: T) -> Self {
        Self::Owned(value)
    }

    /// Takes the value back out of the storage.
    ///
    /// # Errors
    ///
    /// Gives the storage back unchanged if it's vacant or the value is still loaned out (other
    /// [`Loan`]s alive, or a [`LoanMut`] not yet dropped).
    pub fn into_owned(self) -> Result<T, Self> {
        match self {
            Self::Vacant => Err(Self::Vacant),
            Self::Owned(value) => Ok(value),
            Self::Loan(value) => match Arc::try_unwrap(value) {
                Ok(value) => Ok(value),
                Err(value) => Err(Self::Loan(value)),
            },
            Self::LoanMut(value) => {
                let mut guard = value.write();
                let maybe = guard.take();
                drop(guard);
                maybe.ok_or_else(|| Self::LoanMut(value))
            }
        }
    }

    /// Converts the storage into a shared [`Loan`] of its value.
    ///
    /// # Errors
    ///
    /// Gives the storage back unchanged if it's vacant or exclusively loaned.
    pub fn into_loaned(self) -> Result<Loan<T>, Self> {
        match self {
            Self::Vacant => Err(Self::Vacant),
            Self::Owned(value) => Ok(Loan(Arc::new(value))),
            Self::Loan(value) => Ok(Loan(value)),
            Self::LoanMut(value) => {
                let mut guard = value.write();
                let maybe = guard.take();
                drop(guard);
                maybe
                    .map(|value| Loan(Arc::new(value)))
                    .ok_or_else(|| Self::LoanMut(value))
            }
        }
    }

    /// The value, if the storage owns it (no loans out).
    pub fn as_owned_ref(&self) -> Option<&T> {
        match self {
            Self::Owned(value) => Some(value),
            _ => None,
        }
    }

    /// The value mutably, if the storage owns it (no loans out).
    pub fn as_owned_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Owned(value) => Some(value),
            _ => None,
        }
    }

    /// Lends the value out for shared reading, or returns `None` if it's vacant or exclusively
    /// loaned. Any number of shared loans can be out at once.
    pub fn loan(&mut self) -> Option<Loan<T>> {
        let this = std::mem::replace(self, Self::Vacant);
        match this.into_loaned() {
            Ok(loan) => {
                *self = Self::Loan(loan.0.clone());
                Some(loan)
            }
            Err(this) => {
                *self = this;
                None
            }
        }
    }

    /// Lends the value out exclusively, or returns `None` if it's vacant or any loan is out.
    pub fn loan_mut(&mut self) -> Option<LoanMut<T>> {
        let this = std::mem::replace(self, Self::Vacant);
        match this.into_owned() {
            Ok(value) => {
                let outer = SharedLock::new(None);
                *self = Self::LoanMut(outer.clone());
                Some(LoanMut {
                    inner: Some(value),
                    outer,
                })
            }
            Err(this) => {
                *self = this;
                None
            }
        }
    }
}

impl<T> From<T> for LoanStorage<T> {
    fn from(value: T) -> Self {
        Self::Owned(value)
    }
}

impl<T> From<Loan<T>> for LoanStorage<T> {
    fn from(value: Loan<T>) -> Self {
        Self::Loan(value.0)
    }
}
