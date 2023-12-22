pub mod bundle;
pub mod component;
pub mod entity;
pub mod query;
pub mod resource;
pub mod system;
pub mod world;

/// Helper trait for downcasting to `Any`.
pub trait Downcast {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: 'static> Downcast for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
