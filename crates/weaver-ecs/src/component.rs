//! The [`Component`] trait, and the resource store built on [`LoanStorage`].

use std::{
    any::{Any, TypeId},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use weaver_util::prelude::*;

use crate::{
    change_detection::{ChangeDetection, ChangeDetectionMut, ComponentTicks, Tick},
    loan::{Loan, LoanMut, LoanStorage},
};

/// Anything that can be stored on an entity or used as a resource.
///
/// There's nothing to implement: every `Any + Send + Sync` type is a component through a blanket
/// impl. The `as_any*` methods exist so `dyn Component` can be downcast.
pub trait Component: Any + Send + Sync {
    /// Upcasts to `&dyn Any`.
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    /// Upcasts to `&mut dyn Any`.
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);
    /// Upcasts a boxed component to `Box<dyn Any>`.
    fn as_any_box(self: Box<Self>) -> Box<dyn Any + Send + Sync>;
    /// Upcasts an `Arc` component to `Arc<dyn Any>`.
    fn as_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    /// The type's name, for diagnostics.
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
    /// Returns the component as a `T`, or `None` if it's a different type.
    pub fn downcast_ref<T: Component>(&self) -> Option<&T> {
        self.as_any().downcast_ref()
    }

    /// Returns the component as a `&mut T`, or `None` if it's a different type.
    pub fn downcast_mut<T: Component>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut()
    }

    /// Downcasts a boxed component to `Box<T>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the component isn't a `T`.
    pub fn downcast<T: Component>(self: Box<Self>) -> Result<Box<T>> {
        match self.as_any_box().downcast() {
            Ok(t) => Ok(t),
            Err(_) => bail!("Failed to downcast component"),
        }
    }

    /// Downcasts an `Arc` component to `Arc<T>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the component isn't a `T`.
    pub fn downcast_arc<T: Component>(self: Arc<Self>) -> Result<Arc<T>> {
        match self.as_any_arc().downcast() {
            Ok(t) => Ok(t),
            Err(_) => bail!("Failed to downcast component"),
        }
    }
}

/// A heap-allocated, type-erased component.
pub type BoxedComponent = Box<dyn Component>;

pub use crate::component_vec::ComponentVec;

/// A type-keyed store of single values, each with its own change ticks.
///
/// This is where the [`World`](crate::world::World) keeps its resources. Values are lent out as
/// [`Res`] and [`ResMut`] through [`LoanStorage`], so conflicting borrows fail (returning `None`)
/// instead of blocking.
#[derive(Default)]
pub struct ComponentMap {
    map: TypeIdMap<LoanStorage<BoxedComponent>>,
    ticks: TypeIdMap<LoanStorage<ComponentTicks>>,
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
    /// Stores `value`, replacing and returning any previous value of the same type. The ticks are
    /// reset to added and changed at `tick`.
    ///
    /// # Errors
    ///
    /// Returns an error if the old value is currently loaned out. The old value stays in place and
    /// the new one is dropped.
    #[must_use = "This method returns the old component if it exists"]
    pub fn insert_component<T: Component>(&mut self, value: T, tick: Tick) -> Result<Option<T>> {
        self.ticks.insert(
            TypeId::of::<T>(),
            LoanStorage::new(ComponentTicks::new(tick)),
        );
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

    fn get_ticks(&mut self, type_id: TypeId) -> Result<Loan<ComponentTicks>> {
        let storage = self.ticks.get_mut(&type_id);
        let Some(storage) = storage else {
            bail!("Resource does not exist");
        };
        let loan = storage.loan();
        let Some(loan) = loan else {
            bail!("Resource is already mutably borrowed");
        };
        Ok(loan)
    }

    fn get_ticks_mut(&mut self, type_id: TypeId) -> Result<LoanMut<ComponentTicks>> {
        let storage = self.ticks.get_mut(&type_id);
        let Some(storage) = storage else {
            bail!("Resource does not exist");
        };
        let loan = storage.loan_mut();
        let Some(loan) = loan else {
            bail!("Resource is already borrowed");
        };
        Ok(loan)
    }

    /// Borrows the `T` for reading, or returns `None` if there is no `T` or it's mutably borrowed.
    ///
    /// Change detection on the returned [`Res`] compares against `last_run` and `this_run`.
    pub fn get_component<T: Component>(
        &mut self,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Res<T>> {
        match (
            self.loan_component(TypeId::of::<T>()),
            self.get_ticks(TypeId::of::<T>()),
        ) {
            (Ok(loan), Ok(ticks)) => Some(Res {
                loan,
                last_run,
                this_run,
                ticks,
                marker: std::marker::PhantomData,
            }),
            (Err(e), _) => {
                log::debug!("Failed to get resource {:?}: {}", T::type_name(), e);
                None
            }
            (_, Err(e)) => {
                log::debug!("Failed to get resource {:?}: {}", T::type_name(), e);
                None
            }
        }
    }

    /// Borrows the `T` for writing, or returns `None` if there is no `T` or it's already borrowed.
    ///
    /// Writes through the returned [`ResMut`] are stamped with `this_run`.
    pub fn get_component_mut<T: Component>(
        &mut self,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<ResMut<T>> {
        match (
            self.loan_component_mut(TypeId::of::<T>()),
            self.get_ticks_mut(TypeId::of::<T>()),
        ) {
            (Ok(loan), Ok(ticks)) => Some(ResMut {
                loan,
                last_run,
                this_run,
                ticks,
                marker: std::marker::PhantomData,
            }),
            (Err(e), _) => {
                log::debug!("Failed to get mutable resource {:?}: {}", T::type_name(), e);
                None
            }
            (_, Err(e)) => {
                log::debug!("Failed to get mutable resource {:?}: {}", T::type_name(), e);
                None
            }
        }
    }

    /// Whether a `T` is stored, whether or not it's currently borrowed.
    pub fn contains_component<T: Component>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    /// Removes and returns the `T`, or `Ok(None)` if there isn't one.
    ///
    /// # Errors
    ///
    /// Returns an error if the value is currently loaned out. It stays in the map.
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

/// A shared borrow of a resource. Derefs to the resource.
///
/// As a system parameter it makes the system skip its run if the resource is missing or mutably
/// borrowed. Change detection is available through [`ChangeDetection`].
pub struct Res<T: Component> {
    loan: Loan<BoxedComponent>,
    last_run: Tick,
    this_run: Tick,
    ticks: Loan<ComponentTicks>,
    marker: std::marker::PhantomData<T>,
}

impl<T: Component> ChangeDetection for Res<T> {
    fn is_added(&self) -> bool {
        self.ticks.is_added(self.last_run, self.this_run)
    }

    fn is_changed(&self) -> bool {
        self.ticks.is_changed(self.last_run, self.this_run)
    }

    fn last_changed(&self) -> Tick {
        self.ticks.changed
    }
}

impl<T: Component> Deref for Res<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.loan.downcast_ref().unwrap()
    }
}

/// An exclusive borrow of a resource. Derefs to the resource.
///
/// Going through `DerefMut` marks the resource changed;
/// [`ChangeDetectionMut::bypass_change_detection`] avoids that.
pub struct ResMut<T: Component> {
    loan: LoanMut<BoxedComponent>,
    last_run: Tick,
    this_run: Tick,
    ticks: LoanMut<ComponentTicks>,
    marker: std::marker::PhantomData<T>,
}

impl<T: Component> ChangeDetection for ResMut<T> {
    fn is_added(&self) -> bool {
        self.ticks.is_added(self.last_run, self.this_run)
    }

    fn is_changed(&self) -> bool {
        self.ticks.is_changed(self.last_run, self.this_run)
    }

    fn last_changed(&self) -> Tick {
        self.ticks.changed
    }
}

impl<T: Component> ChangeDetectionMut for ResMut<T> {
    type Inner = T;

    fn set_changed(&mut self) {
        self.ticks.set_changed(self.this_run);
    }

    fn set_last_changed(&mut self, tick: Tick) {
        self.ticks.set_changed(tick);
    }

    fn bypass_change_detection(&mut self) -> &mut Self::Inner {
        self.loan.downcast_mut().unwrap()
    }
}

impl<T: Component> Deref for ResMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.loan.downcast_ref().unwrap()
    }
}

impl<T: Component> DerefMut for ResMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        self.loan.downcast_mut().unwrap()
    }
}
