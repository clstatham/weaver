//! Much of the inspiration and implementation of this module is based on the `broomdog` crate, particularly the `Loan`, `LoanMut`, `LoanStorage`, and `ComponentMap` types.
//! <https://github.com/schell/broomdog/>

use std::{
    any::{Any, TypeId},
    ops::{Deref, DerefMut},
};

use weaver_util::{bail, Result, TypeIdMap};

use crate::loan::{Loan, LoanMut, LoanStorage};

pub type BoxedComponent = Box<dyn Any + Send + Sync>;

#[derive(Default)]
pub struct ComponentMap {
    map: TypeIdMap<LoanStorage<BoxedComponent>>,
}

impl Deref for ComponentMap {
    type Target = TypeIdMap<LoanStorage<BoxedComponent>>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl DerefMut for ComponentMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl ComponentMap {
    #[must_use = "This method returns the old component if it exists"]
    pub fn insert_component<T: Any + Send + Sync>(&mut self, value: T) -> Result<Option<T>> {
        if let Some(old) = self
            .map
            .insert(TypeId::of::<T>(), LoanStorage::new(Box::new(value)))
        {
            match old.into_owned() {
                Ok(old) => Ok(Some(*old.downcast().unwrap())),
                Err(old) => {
                    self.map.insert(TypeId::of::<T>(), old);
                    bail!("Failed to downcast old component")
                }
            }
        } else {
            Ok(None)
        }
    }

    fn loan_component(&mut self, type_id: TypeId) -> Result<Loan<BoxedComponent>> {
        // self.map.get_mut(&type_id)?.loan()
        let storage = self.map.get_mut(&type_id);
        let Some(storage) = storage else {
            bail!("Resource does not exist");
        };
        let loan = storage.loan();
        let Some(loan) = loan else {
            bail!("Resource is already mutably borrowed");
        };
        Ok(loan)
    }

    fn loan_component_mut(&mut self, type_id: TypeId) -> Result<LoanMut<BoxedComponent>> {
        let storage = self.map.get_mut(&type_id);
        let Some(storage) = storage else {
            bail!("Resource does not exist");
        };
        let loan = storage.loan_mut();
        let Some(loan) = loan else {
            bail!("Resource is already borrowed");
        };
        Ok(loan)
    }

    async fn loan_component_patient(&mut self, type_id: TypeId) -> Result<Loan<BoxedComponent>> {
        let storage = self.map.get_mut(&type_id);
        let Some(storage) = storage else {
            bail!("Resource does not exist");
        };
        let loan = storage.wait_for_loan().await;
        Ok(loan)
    }

    async fn loan_component_mut_patient(
        &mut self,
        type_id: TypeId,
    ) -> Result<LoanMut<BoxedComponent>> {
        let storage = self.map.get_mut(&type_id);
        let Some(storage) = storage else {
            bail!("Resource does not exist");
        };
        let loan = storage.wait_for_loan_mut().await;
        Ok(loan)
    }

    pub fn get_component<T: Any + Send + Sync>(&mut self) -> Option<Ref<T>> {
        match self.loan_component(TypeId::of::<T>()) {
            Ok(loan) => Some(Ref {
                loan,
                marker: std::marker::PhantomData,
            }),
            Err(e) => {
                log::debug!(
                    "Failed to get resource {:?}: {}",
                    std::any::type_name::<T>(),
                    e
                );
                None
            }
        }
    }

    pub fn get_component_mut<T: Any + Send + Sync>(&mut self) -> Option<Mut<T>> {
        match self.loan_component_mut(TypeId::of::<T>()) {
            Ok(loan) => Some(Mut {
                loan,
                marker: std::marker::PhantomData,
            }),
            Err(e) => {
                log::debug!(
                    "Failed to get mutable resource {:?}: {}",
                    std::any::type_name::<T>(),
                    e
                );
                None
            }
        }
    }

    pub async fn wait_for_component<T: Any + Send + Sync>(&mut self) -> Option<Ref<T>> {
        let loan = self
            .loan_component_patient(TypeId::of::<T>())
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "Failed to get resource {:?}: {}",
                    std::any::type_name::<T>(),
                    e
                );
            });
        Some(Ref {
            loan,
            marker: std::marker::PhantomData,
        })
    }

    pub async fn wait_for_component_mut<T: Any + Send + Sync>(&mut self) -> Option<Mut<T>> {
        let loan = self
            .loan_component_mut_patient(TypeId::of::<T>())
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "Failed to get mutable resource {:?}: {}",
                    std::any::type_name::<T>(),
                    e
                );
            });
        Some(Mut {
            loan,
            marker: std::marker::PhantomData,
        })
    }

    pub fn contains_component<T: Any + Send + Sync>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    pub fn remove_component<T: Any + Send + Sync>(&mut self) -> Result<Option<T>> {
        if let Some(value) = self.map.remove(&TypeId::of::<T>()) {
            match value.into_owned() {
                Ok(value) => Ok(Some(*value.downcast().unwrap())),
                Err(value) => {
                    self.map.insert(TypeId::of::<T>(), value);
                    bail!("Failed to downcast old component")
                }
            }
        } else {
            Ok(None)
        }
    }
}

pub struct Ref<T: Any + Send + Sync> {
    loan: Loan<BoxedComponent>,
    marker: std::marker::PhantomData<T>,
}

impl<T: Any + Send + Sync> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.loan.downcast_ref().unwrap()
    }
}

pub struct Mut<T: Any + Send + Sync> {
    loan: LoanMut<BoxedComponent>,
    marker: std::marker::PhantomData<T>,
}

impl<T: Any + Send + Sync> Deref for Mut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.loan.downcast_ref().unwrap()
    }
}

impl<T: Any + Send + Sync> DerefMut for Mut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.loan.downcast_mut().unwrap()
    }
}

pub struct Res<T: Any + Send + Sync>(pub(crate) Ref<T>);
pub struct ResMut<T: Any + Send + Sync>(pub(crate) Mut<T>);

impl<T: Any + Send + Sync> Deref for Res<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Any + Send + Sync> Deref for ResMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Any + Send + Sync> DerefMut for ResMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
