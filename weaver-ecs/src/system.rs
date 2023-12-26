use crate::World;

/// Notes:
///
/// - System trait implementors are actually storage for the system's arguments.
///   The system macro will generate a struct that implements System and contains
///   the arguments as fields.
pub trait System {
    fn run(&self, world: &World);
}
