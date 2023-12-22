use super::Downcast;

pub trait Component: Downcast + Send + Sync + 'static {
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

// impl<T: Downcast + Send + Sync + 'static> Component for T {}
