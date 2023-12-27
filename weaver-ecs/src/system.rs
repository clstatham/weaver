use crate::World;

pub trait System {
    fn run(&self, world: &World);
}
