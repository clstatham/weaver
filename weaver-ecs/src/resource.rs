use crate::component::Downcast;

pub trait Resource: Downcast + Send + Sync + 'static {}
