use super::world::World;

pub trait System
where
    Self: 'static,
{
    fn run(&self, world: &mut World, delta: std::time::Duration);
}

impl<F> System for F
where
    F: Fn(&mut World, std::time::Duration) + 'static,
{
    fn run(&self, world: &mut World, delta: std::time::Duration) {
        self(world, delta)
    }
}
