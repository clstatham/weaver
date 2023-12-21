use super::world::World;

pub trait System
where
    Self: 'static,
{
    fn run(&mut self, world: &mut World, delta: std::time::Duration);
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

impl<F> System for F
where
    F: FnMut(&mut World, std::time::Duration) + 'static,
{
    fn run(&mut self, world: &mut World, delta: std::time::Duration) {
        self(world, delta)
    }
}
