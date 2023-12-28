use std::cell::RefCell;

use rustc_hash::FxHashMap;

use crate::Entity;

pub trait Downcast: std::any::Any {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: std::any::Any> Downcast for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A component is a data structure that can be attached to an entity.
///
/// # Safety
/// This trait is only intended to be implemented by the `#[derive(Component)]` macro.
pub unsafe trait Component: Downcast + Send + Sync {
    fn component_id() -> u64
    where
        Self: Sized;
}

/// An indicator of whether a component is borrowed for reading and/or writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorrowStatus {
    /// The component is not borrowed.
    None,
    /// The component is immutably borrowed for reading.
    Read,
    /// The component is mutably borrowed for reading and writing.
    Write,
}

#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorrowError {
    #[error("The component is already immutably borrowed")]
    Read,
    #[error("The component is already mutably borrowed")]
    Write,
}

#[derive(Default)]
pub struct BorrowIntent {
    pub(crate) intended_borrow: RefCell<FxHashMap<Entity, FxHashMap<u64, BorrowStatus>>>,
}

impl BorrowIntent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets the borrow intent for the next update tick.
    ///
    /// # Safety
    /// This function is unsafe because it is not guaranteed that all borrows have been released.
    /// This function should only be called at the beginning of a update tick.
    /// If this function is called in the middle of a update tick, it may cause the borrows to fail, likely resulting in a panic.
    pub unsafe fn reset(&mut self) {
        self.reset_internal();
    }

    #[doc(hidden)]
    fn reset_internal(&mut self) {
        for component_status in self.intended_borrow.borrow_mut().values_mut() {
            for status in component_status.values_mut() {
                *status = BorrowStatus::None;
            }
        }
    }

    /// Returns whether the component can be borrowed for reading.
    pub fn can_read<T: Component>(&self, entity: Entity) -> bool {
        if let Some(component_status) = self.intended_borrow.borrow().get(&entity) {
            if let Some(status) = component_status.get(&T::component_id()) {
                match status {
                    BorrowStatus::None => true,
                    BorrowStatus::Read => true,
                    BorrowStatus::Write => false,
                }
            } else {
                true
            }
        } else {
            true
        }
    }

    /// Returns whether the component can be borrowed for writing.
    pub fn can_write<T: Component>(&self, entity: Entity) -> bool {
        if let Some(component_status) = self.intended_borrow.borrow().get(&entity) {
            if let Some(status) = component_status.get(&T::component_id()) {
                match status {
                    BorrowStatus::None => true,
                    BorrowStatus::Read => false,
                    BorrowStatus::Write => false,
                }
            } else {
                true
            }
        } else {
            true
        }
    }

    /// Attempts to mark the component as borrowed for reading.
    pub fn try_read<T: Component>(&self, entity: Entity) -> Option<Result<(), BorrowError>> {
        if let Some(component_status) = self.intended_borrow.borrow_mut().get_mut(&entity) {
            if let Some(status) = component_status.get_mut(&T::component_id()) {
                match status {
                    BorrowStatus::None => {
                        *status = BorrowStatus::Read;
                        Some(Ok(()))
                    }
                    BorrowStatus::Read => Some(Ok(())),
                    BorrowStatus::Write => Some(Err(BorrowError::Write)),
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Attempts to mark the component as borrowed for writing.
    pub fn try_write<T: Component>(&self, entity: Entity) -> Option<Result<(), BorrowError>> {
        if let Some(component_status) = self.intended_borrow.borrow_mut().get_mut(&entity) {
            if let Some(status) = component_status.get_mut(&T::component_id()) {
                match status {
                    BorrowStatus::None => {
                        *status = BorrowStatus::Write;
                        Some(Ok(()))
                    }
                    BorrowStatus::Read => Some(Err(BorrowError::Read)),
                    BorrowStatus::Write => Some(Err(BorrowError::Write)),
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}
