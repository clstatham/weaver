use super::Downcast;

pub trait Component: Downcast + Send + Sync + 'static {}

impl<T: Downcast + Send + Sync + 'static> Component for T {}
