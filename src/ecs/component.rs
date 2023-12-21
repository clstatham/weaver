use super::{
    query::{ReadResult, WriteResult},
    world::World,
    Downcast,
};

pub trait Component: Downcast + Send + Sync + 'static {}

impl<T: Downcast + Send + Sync + 'static> Component for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component() {
        let component = 42;
        let component = &component as &dyn Component;
        assert_eq!(component.as_any().downcast_ref::<i32>(), Some(&42));
    }
}
