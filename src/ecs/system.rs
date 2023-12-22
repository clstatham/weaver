use super::world::World;

pub trait ExclusiveSystem
where
    Self: 'static,
{
    fn run_exclusive(&mut self, world: &mut World, delta: std::time::Duration);
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

impl<F> ExclusiveSystem for F
where
    F: FnMut(&mut World, std::time::Duration) + 'static,
{
    fn run_exclusive(&mut self, world: &mut World, delta: std::time::Duration) {
        self(world, delta)
    }
}

/// Systems that, instead of taking a mutable reference to the `World`, take a Read or Write query.
pub trait System
where
    Self: 'static,
{
    fn run(&mut self, world: &mut World, delta: std::time::Duration);
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}
