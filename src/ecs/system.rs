use super::world::World;

pub trait System
where
    Self: 'static,
{
    fn run(&self, world: &mut World);
}

impl<F> System for F
where
    F: Fn(&mut World) + 'static,
{
    fn run(&self, world: &mut World) {
        self(world)
    }
}
