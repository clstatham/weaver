use std::sync::Arc;

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

    fn component_id_sized(&self) -> u64
    where
        Self: Sized,
    {
        Self::component_id()
    }
}

unsafe impl<T: Component> Component for Option<T> {
    fn component_id() -> u64
    where
        Self: Sized,
    {
        T::component_id()
    }
}

unsafe impl<T: Component> Component for Vec<T> {
    fn component_id() -> u64
    where
        Self: Sized,
    {
        T::component_id()
    }
}

unsafe impl<T: Component> Component for Arc<T> {
    fn component_id() -> u64
    where
        Self: Sized,
    {
        T::component_id()
    }
}
