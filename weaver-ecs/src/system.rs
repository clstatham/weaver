use crate::World;

pub trait System {
    fn run(&self, world: &World);
    fn components_read(&self) -> Vec<u64>;
    fn components_written(&self) -> Vec<u64>;
}
