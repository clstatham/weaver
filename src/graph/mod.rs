/// An operation that can be performed in a [GraphSystemLogic].
pub enum GraphOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Set,
    Get,
    Print,
    Subgraph(Box<GraphSystemLogic>),
}

/// A type of dynamic system logic that runs a runtime-definable series of operations on Components.
pub struct GraphSystemLogic {
    operations: Vec<GraphOperation>,
}

impl GraphSystemLogic {
    /// Creates a new [GraphSystemLogic] with the given operations.
    pub fn new(operations: Vec<GraphOperation>) -> Self {
        GraphSystemLogic { operations }
    }
}
