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
#[cfg_attr(feature = "serde", typetag::serde(tag = "type"))]
pub trait Component: Downcast + Send + Sync + super::StaticId + 'static {}
