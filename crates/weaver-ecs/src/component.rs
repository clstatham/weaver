//! Much of the inspiration and implementation of this module is based on the `broomdog` crate, particularly the `Loan`, `LoanMut`, `LoanStorage`, and `ComponentMap` types.
//! <https://github.com/schell/broomdog/>

use std::{
    any::{Any, TypeId},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use any_vec::AnyVec;
use weaver_util::{bail, Result, TypeIdMap};

use crate::loan::{Loan, LoanMut, LoanStorage};

pub trait Component: Any + Send + Sync {
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);
    fn as_any_box(self: Box<Self>) -> Box<dyn Any + Send + Sync>;
    fn as_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        std::any::type_name::<Self>()
    }
}
impl<T: Any + Send + Sync> Component for T {
    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }
    fn as_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

impl dyn Component {
    pub fn downcast_ref<T: Component>(&self) -> Option<&T> {
        self.as_any().downcast_ref()
    }

    pub fn downcast_mut<T: Component>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut()
    }

    pub fn downcast<T: Component>(self: Box<Self>) -> Result<Box<T>> {
        match self.as_any_box().downcast() {
            Ok(t) => Ok(t),
            Err(_) => bail!("Failed to downcast component"),
        }
    }

    pub fn downcast_arc<T: Component>(self: Arc<Self>) -> Result<Arc<T>> {
        match self.as_any_arc().downcast() {
            Ok(t) => Ok(t),
            Err(_) => bail!("Failed to downcast component"),
        }
    }
}

pub type BoxedComponent = Box<dyn Component>;

pub type ComponentVec = AnyVec<dyn Send + Sync>;

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
    pub fn insert_component<T: Component>(&mut self, value: T) -> Result<Option<T>> {
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

    pub fn get_component<T: Component>(&mut self) -> Option<Ref<T>> {
        match self.loan_component(TypeId::of::<T>()) {
            Ok(loan) => Some(Ref {
                loan,
                marker: std::marker::PhantomData,
            }),
            Err(e) => {
                log::debug!("Failed to get resource {:?}: {}", T::type_name(), e);
                None
            }
        }
    }

    pub fn get_component_mut<T: Component>(&mut self) -> Option<Mut<T>> {
        match self.loan_component_mut(TypeId::of::<T>()) {
            Ok(loan) => Some(Mut {
                loan,
                marker: std::marker::PhantomData,
            }),
            Err(e) => {
                log::debug!("Failed to get mutable resource {:?}: {}", T::type_name(), e);
                None
            }
        }
    }

    pub async fn wait_for_component<T: Component>(&mut self) -> Option<Ref<T>> {
        let loan = self
            .loan_component_patient(TypeId::of::<T>())
            .await
            .unwrap_or_else(|e| {
                panic!("Failed to get resource {:?}: {}", T::type_name(), e);
            });
        Some(Ref {
            loan,
            marker: std::marker::PhantomData,
        })
    }

    pub async fn wait_for_component_mut<T: Component>(&mut self) -> Option<Mut<T>> {
        let loan = self
            .loan_component_mut_patient(TypeId::of::<T>())
            .await
            .unwrap_or_else(|e| {
                panic!("Failed to get mutable resource {:?}: {}", T::type_name(), e);
            });
        Some(Mut {
            loan,
            marker: std::marker::PhantomData,
        })
    }

    pub fn contains_component<T: Component>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    pub fn remove_component<T: Component>(&mut self) -> Result<Option<T>> {
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

pub struct Ref<T: Component> {
    loan: Loan<BoxedComponent>,
    marker: std::marker::PhantomData<T>,
}

impl<T: Component> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.loan.downcast_ref().unwrap()
    }
}

pub struct Mut<T: Component> {
    loan: LoanMut<BoxedComponent>,
    marker: std::marker::PhantomData<T>,
}

impl<T: Component> Deref for Mut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.loan.downcast_ref().unwrap()
    }
}

impl<T: Component> DerefMut for Mut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.loan.downcast_mut().unwrap()
    }
}

pub struct Res<T: Component>(pub(crate) Ref<T>);
pub struct ResMut<T: Component>(pub(crate) Mut<T>);

impl<T: Component> Deref for Res<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Component> Deref for ResMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Component> DerefMut for ResMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
