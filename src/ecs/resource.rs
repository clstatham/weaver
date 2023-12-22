use super::Downcast;

pub trait Resource
where
    Self: Downcast + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}
