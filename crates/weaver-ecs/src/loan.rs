use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use weaver_util::SharedLock;

pub struct Loan<T>(Arc<T>);

impl<T> Loan<T> {
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

pub enum LoanStorage<T> {
    Vacant,
    Owned(T),
    Loan(Arc<T>),
    LoanMut(SharedLock<Option<T>>),
}

impl<T> Default for LoanStorage<T> {
    fn default() -> Self {
        Self::Vacant
    }
}

impl<T> LoanStorage<T> {
    pub fn new(value: T) -> Self {
        Self::Owned(value)
    }

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

    pub fn as_owned_ref(&self) -> Option<&T> {
        match self {
            Self::Owned(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_owned_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Owned(value) => Some(value),
            _ => None,
        }
    }

    pub fn loan(&mut self) -> Option<Loan<T>> {
        let this = std::mem::replace(self, Self::Vacant);
        match this.into_loaned() {
            Ok(loan) => {
                *self = Self::Loan(loan.0.clone());
                Some(loan)
            }
            Err(this) => {
                log::debug!("Failed to get loan on {}", std::any::type_name::<T>());
                *self = this;
                None
            }
        }
    }

    pub async fn wait_for_loan(&mut self) -> Loan<T> {
        loop {
            if let Some(loan) = self.loan() {
                return loan;
            }
            tokio::task::yield_now().await;
        }
    }

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
                log::debug!(
                    "Failed to get mutable loan on {}",
                    std::any::type_name::<T>()
                );
                *self = this;
                None
            }
        }
    }

    pub async fn wait_for_loan_mut(&mut self) -> LoanMut<T> {
        loop {
            if let Some(loan) = self.loan_mut() {
                return loan;
            }
            tokio::task::yield_now().await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_loan() {
        let mut storage = LoanStorage::new(42);
        let loan = storage.loan().unwrap();
        assert_eq!(*loan, 42);
        drop(loan);
        let loan = storage.wait_for_loan().await;
        assert_eq!(*loan, 42);
    }

    #[tokio::test]
    async fn test_loan_mut() {
        let mut storage = LoanStorage::new(42);
        let mut loan = storage.loan_mut().unwrap();
        assert_eq!(*loan, 42);
        *loan = 43;
        drop(loan);
        let mut loan = storage.wait_for_loan_mut().await;
        assert_eq!(*loan, 43);
        *loan = 44;
        drop(loan);
        let loan = storage.loan().unwrap();
        assert_eq!(*loan, 44);
    }
}
